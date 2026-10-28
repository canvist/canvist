//! Deepgram's streaming `listen` API.
//!
//! Deepgram takes raw 16 kHz PCM16 frames and finds turns itself. Interim
//! results (`is_final: false`) rewrite the current segment, and a final
//! result closes it, so the relay's voice activity detection is not needed.
//! On `stop`, `Finalize` flushes the last segment and `CloseStream` asks
//! Deepgram to close once everything is sent.

use std::time::Duration;

use serde::Deserialize;
use serde_json::json;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::client::Request;

use super::Adapter;
use super::Outbox;
use super::ProviderError;
use super::SessionOptions;
use super::error_message;
use super::query_value;
use super::request;
use crate::audio::encode_pcm16;
use crate::config::ProviderConfig;
use crate::config::Secret;

/// Deepgram closes a stream after 10 seconds without audio; a keepalive
/// inside that keeps a paused session open.
const KEEPALIVE: Duration = Duration::from_secs(5);

/// A Deepgram streaming session.
#[derive(Debug)]
pub struct Deepgram {
	url: String,
	key: Option<Secret>,
	segment: u32,
	/// Whether the current segment has a draft the client is showing.
	drafted: bool,
	last_sent: Instant,
	finishing: bool,
	closed: bool,
}

#[derive(Deserialize)]
struct Results {
	#[serde(default)]
	is_final: bool,
	channel: Channel,
}

#[derive(Deserialize)]
struct Channel {
	alternatives: Vec<Alternative>,
}

#[derive(Deserialize)]
struct Alternative {
	transcript: String,
}

impl Deepgram {
	/// A session for `config`, transcribing `options.language` when set.
	pub fn new(config: &ProviderConfig, options: &SessionOptions) -> Self {
		let mut url = format!(
			"{}/v1/listen?model={}&encoding=linear16&sample_rate={}&channels=1&\
			 interim_results=true&smart_format=true&punctuate=true",
			config.url,
			query_value(&config.model),
			canvist_dictation::SAMPLE_RATE,
		);

		if let Some(language) = &options.language {
			url.push_str("&language=");
			url.push_str(&query_value(language));
		}

		Self {
			url,
			key: config.key.clone(),
			segment: 0,
			drafted: false,
			last_sent: Instant::now(),
			finishing: false,
			closed: false,
		}
	}

	fn results(&mut self, results: Results, out: &mut Outbox) {
		let transcript = results
			.channel
			.alternatives
			.into_iter()
			.next()
			.map(|alternative| alternative.transcript)
			.unwrap_or_default();
		let transcript = transcript.trim();

		if !results.is_final {
			if !transcript.is_empty() {
				out.partial(self.segment, transcript);
				self.drafted = true;
			}

			return;
		}

		// An empty final closes a segment only when the client is showing a
		// draft of it; otherwise it is silence and numbers nothing.
		if transcript.is_empty() && !self.drafted {
			return;
		}

		out.final_text(self.segment, transcript);
		self.segment += 1;
		self.drafted = false;
	}
}

impl Adapter for Deepgram {
	fn request(&self) -> Result<Request, ProviderError> {
		request(&self.url, "Token", self.key.as_ref())
	}

	fn open(&mut self, _out: &mut Outbox) {
		self.last_sent = Instant::now();
	}

	fn audio(&mut self, samples: &[i16], out: &mut Outbox) {
		out.messages.push(Message::binary(encode_pcm16(samples)));
		self.last_sent = Instant::now();
	}

	fn end_of_turn(&mut self, _now: Instant, _out: &mut Outbox) {}

	fn finish(&mut self, out: &mut Outbox) {
		out.send(&json!({ "type": "Finalize" }));
		out.send(&json!({ "type": "CloseStream" }));
		self.finishing = true;
	}

	fn receive(&mut self, text: &str, out: &mut Outbox) -> Result<(), ProviderError> {
		let event: serde_json::Value =
			serde_json::from_str(text).map_err(|_| ProviderError::Malformed)?;

		match event["type"].as_str().unwrap_or_default() {
			"Results" => {
				let results =
					serde_json::from_value(event).map_err(|_| ProviderError::Malformed)?;
				self.results(results, out);
			}
			"Error" => return Err(ProviderError::Remote(error_message(&event))),
			_ => {}
		}

		Ok(())
	}

	fn deadline(&self) -> Option<Instant> {
		(!self.finishing).then_some(self.last_sent + KEEPALIVE)
	}

	fn tick(&mut self, now: Instant, out: &mut Outbox) {
		if self.finishing || now < self.last_sent + KEEPALIVE {
			return;
		}

		out.send(&json!({ "type": "KeepAlive" }));
		self.last_sent = now;
	}

	fn is_finished(&self) -> bool {
		self.closed
	}

	fn closed(&mut self) -> Result<(), ProviderError> {
		if !self.finishing {
			return Err(ProviderError::Closed);
		}

		self.closed = true;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::config::ProviderKind;
	use crate::provider::Adapter;
	use crate::provider::Event;

	fn adapter() -> Deepgram {
		let config = ProviderConfig::new(ProviderKind::Deepgram, Some(Secret::new("dg-key")));
		let options = SessionOptions {
			language: Some("en-US".to_owned()),
		};
		Deepgram::new(&config, &options)
	}

	fn result(adapter: &mut Deepgram, transcript: &str, is_final: bool) -> Vec<Event> {
		let mut out = Outbox::default();
		let event = json!({
			"type": "Results",
			"is_final": is_final,
			"channel": { "alternatives": [{ "transcript": transcript, "confidence": 0.9 }] },
		});
		adapter
			.receive(&event.to_string(), &mut out)
			.expect("accepted");
		out.events
	}

	#[test]
	fn the_request_asks_for_interim_16_khz_results() {
		let request = adapter().request().expect("valid");

		assert_eq!(
			request.uri().to_string(),
			"wss://api.deepgram.com/v1/listen?model=nova-3&encoding=linear16&sample_rate=16000&\
			 channels=1&interim_results=true&smart_format=true&punctuate=true&language=en-US"
		);
		assert_eq!(request.headers()["authorization"], "Token dg-key");
	}

	#[test]
	fn audio_is_sent_as_binary_pcm16() {
		let mut out = Outbox::default();
		adapter().audio(&[1, -1], &mut out);

		assert_eq!(out.messages, [Message::binary(vec![1, 0, 255, 255])]);
	}

	#[test]
	fn interim_results_draft_and_finals_advance() {
		let mut adapter = adapter();

		assert_eq!(
			result(&mut adapter, "the meeting", false),
			[Event::Partial {
				segment: 0,
				text: "the meeting".to_owned()
			}]
		);
		assert_eq!(
			result(&mut adapter, "The meeting is on Tuesday.", true),
			[Event::Final {
				segment: 0,
				text: "The meeting is on Tuesday.".to_owned()
			}]
		);
		assert!(
			result(&mut adapter, "", true).is_empty(),
			"silence numbers nothing"
		);
		assert!(result(&mut adapter, " ", false).is_empty());
		assert_eq!(
			result(&mut adapter, "Thanks.", true),
			[Event::Final {
				segment: 1,
				text: "Thanks.".to_owned()
			}]
		);
	}

	#[test]
	fn an_empty_final_still_closes_a_drafted_segment() {
		let mut adapter = adapter();
		result(&mut adapter, "um", false);

		assert_eq!(
			result(&mut adapter, "", true),
			[Event::Final {
				segment: 0,
				text: String::new()
			}]
		);
	}

	#[test]
	fn stop_finalizes_then_closes_the_stream() {
		let mut adapter = adapter();
		let mut out = Outbox::default();
		assert!(matches!(adapter.closed(), Err(ProviderError::Closed)));

		adapter.finish(&mut out);
		assert_eq!(
			out.messages,
			[
				Message::text(r#"{"type":"Finalize"}"#),
				Message::text(r#"{"type":"CloseStream"}"#),
			]
		);
		assert!(!adapter.is_finished(), "Deepgram closes when it is done");
		assert!(adapter.closed().is_ok());
		assert!(adapter.is_finished());
	}

	#[test]
	fn quiet_streams_get_keepalives() {
		let mut adapter = adapter();
		let deadline = adapter.deadline().expect("streaming");
		let mut out = Outbox::default();

		adapter.tick(deadline - Duration::from_millis(1), &mut out);
		assert!(out.messages.is_empty());

		adapter.tick(deadline, &mut out);
		assert_eq!(out.messages, [Message::text(r#"{"type":"KeepAlive"}"#)]);
		assert!(adapter.deadline() > Some(deadline));
	}
}
