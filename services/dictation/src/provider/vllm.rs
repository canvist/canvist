//! vLLM's `/v1/realtime` dialect, for local streaming models such as
//! Voxtral Realtime and Qwen3-ASR.
//!
//! The session names its model, a first commit starts generation, audio is
//! appended at 16 kHz, and a commit with `final: true` ends the stream.
//! The server only ever appends deltas to one running transcript, so the
//! relay cuts it into segments: when voice activity detection hears a pause,
//! the segment stays open for a short grace period to collect the deltas
//! still on their way, then closes.

use std::time::Duration;

use serde_json::json;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::handshake::client::Request;

use super::Adapter;
use super::Outbox;
use super::ProviderError;
use super::base64_pcm16;
use super::error_message;
use super::request;
use crate::config::ProviderConfig;
use crate::config::Secret;

/// A vLLM realtime session.
#[derive(Debug)]
pub struct Vllm {
	url: String,
	model: String,
	key: Option<Secret>,
	grace: Duration,
	segment: u32,
	/// The current segment's deltas.
	text: String,
	/// When the current segment closes, after a pause.
	close_at: Option<Instant>,
	finishing: bool,
	done: bool,
}

impl Vllm {
	/// A session for `config`.
	pub fn new(config: &ProviderConfig) -> Self {
		Self {
			url: format!("{}/v1/realtime", config.url),
			model: config.model.clone(),
			key: config.key.clone(),
			grace: config.segment_grace,
			segment: 0,
			text: String::new(),
			close_at: None,
			finishing: false,
			done: false,
		}
	}

	/// Append `delta` to the current segment and draft it.
	fn delta(&mut self, delta: &str, out: &mut Outbox) {
		self.text.push_str(delta);

		if !self.text.trim().is_empty() {
			out.partial(self.segment, &self.text);
		}
	}

	/// Close the current segment, when it holds any words.
	fn close_segment(&mut self, out: &mut Outbox) {
		self.close_at = None;

		if self.text.trim().is_empty() {
			self.text.clear();

			return;
		}

		out.final_text(self.segment, &self.text);
		self.segment += 1;
		self.text.clear();
	}
}

impl Adapter for Vllm {
	fn request(&self) -> Result<Request, ProviderError> {
		request(&self.url, "Bearer", self.key.as_ref())
	}

	fn open(&mut self, out: &mut Outbox) {
		out.send(&json!({ "type": "session.update", "model": self.model }));
		out.send(&json!({ "type": "input_audio_buffer.commit" }));
	}

	fn audio(&mut self, samples: &[i16], out: &mut Outbox) {
		out.send(&json!({
			"type": "input_audio_buffer.append",
			"audio": base64_pcm16(samples),
		}));
	}

	fn end_of_turn(&mut self, now: Instant, _out: &mut Outbox) {
		self.close_at.get_or_insert(now + self.grace);
	}

	fn finish(&mut self, out: &mut Outbox) {
		out.send(&json!({ "type": "input_audio_buffer.commit", "final": true }));
		self.finishing = true;
	}

	fn receive(&mut self, text: &str, out: &mut Outbox) -> Result<(), ProviderError> {
		let event: serde_json::Value =
			serde_json::from_str(text).map_err(|_| ProviderError::Malformed)?;

		match event["type"].as_str().unwrap_or_default() {
			"transcription.delta" => self.delta(event["delta"].as_str().unwrap_or_default(), out),
			"transcription.done" => {
				self.close_segment(out);
				self.done = self.finishing;
			}
			"error" => return Err(ProviderError::Remote(error_message(&event))),
			_ => {}
		}

		Ok(())
	}

	fn deadline(&self) -> Option<Instant> {
		self.close_at
	}

	fn tick(&mut self, now: Instant, out: &mut Outbox) {
		if self.close_at.is_some_and(|close_at| close_at <= now) {
			self.close_segment(out);
		}
	}

	fn is_finished(&self) -> bool {
		self.done
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::config::ProviderKind;
	use crate::provider::Event;

	fn adapter() -> Vllm {
		Vllm::new(&ProviderConfig::new(ProviderKind::Vllm, None))
	}

	fn sent(out: &Outbox) -> Vec<serde_json::Value> {
		out.messages
			.iter()
			.map(|message| serde_json::from_str(message.to_text().expect("text")).expect("JSON"))
			.collect()
	}

	fn delta(adapter: &mut Vllm, delta: &str) -> Vec<Event> {
		let mut out = Outbox::default();
		let event = json!({ "type": "transcription.delta", "delta": delta });
		adapter
			.receive(&event.to_string(), &mut out)
			.expect("accepted");
		out.events
	}

	#[test]
	fn the_session_names_its_model_then_starts() {
		let mut adapter = adapter();
		let mut out = Outbox::default();
		adapter.open(&mut out);
		adapter.audio(&[1, 2], &mut out);
		adapter.finish(&mut out);

		assert_eq!(
			adapter.request().expect("valid").uri().to_string(),
			"ws://localhost:8000/v1/realtime"
		);
		assert!(
			adapter
				.request()
				.expect("valid")
				.headers()
				.get("authorization")
				.is_none()
		);
		assert_eq!(
			sent(&out),
			[
				json!({ "type": "session.update", "model": "mistralai/Voxtral-Mini-4B-Realtime-2602" }),
				json!({ "type": "input_audio_buffer.commit" }),
				json!({ "type": "input_audio_buffer.append", "audio": "AQACAA==" }),
				json!({ "type": "input_audio_buffer.commit", "final": true }),
			]
		);
	}

	#[test]
	fn deltas_append_and_a_pause_closes_the_segment_after_its_grace() {
		let mut adapter = adapter();
		let start = Instant::now();

		delta(&mut adapter, " the");
		assert_eq!(
			delta(&mut adapter, " meeting"),
			[Event::Partial {
				segment: 0,
				text: "the meeting".to_owned()
			}]
		);

		let mut out = Outbox::default();
		adapter.end_of_turn(start, &mut out);
		assert_eq!(adapter.deadline(), Some(start + Duration::from_millis(600)));

		// A trailing delta inside the grace period joins the segment.
		delta(&mut adapter, " is on.");
		adapter.tick(start + Duration::from_millis(599), &mut out);
		assert!(out.events.is_empty());

		adapter.tick(start + Duration::from_millis(600), &mut out);
		assert_eq!(
			out.events,
			[Event::Final {
				segment: 0,
				text: "the meeting is on.".to_owned()
			}]
		);
		assert_eq!(adapter.deadline(), None);
		assert_eq!(
			delta(&mut adapter, " Next"),
			[Event::Partial {
				segment: 1,
				text: "Next".to_owned()
			}]
		);
	}

	#[test]
	fn a_silent_pause_numbers_nothing() {
		let mut adapter = adapter();
		let mut out = Outbox::default();
		let start = Instant::now();
		adapter.end_of_turn(start, &mut out);
		adapter.tick(start + Duration::from_secs(1), &mut out);

		assert!(out.events.is_empty());
		assert_eq!(
			delta(&mut adapter, "Hi"),
			[Event::Partial {
				segment: 0,
				text: "Hi".to_owned()
			}]
		);
	}

	#[test]
	fn done_after_stop_closes_the_last_segment_and_finishes() {
		let mut adapter = adapter();
		let mut out = Outbox::default();
		delta(&mut adapter, "last words");
		adapter.finish(&mut out);
		assert!(!adapter.is_finished());

		let done = json!({ "type": "transcription.done", "text": "last words" });
		adapter
			.receive(&done.to_string(), &mut out)
			.expect("accepted");

		assert_eq!(
			out.events,
			[Event::Final {
				segment: 0,
				text: "last words".to_owned()
			}]
		);
		assert!(adapter.is_finished());
	}

	#[test]
	fn errors_end_the_session() {
		let mut out = Outbox::default();
		let error = adapter()
			.receive(r#"{"type":"error","error":"model not loaded"}"#, &mut out)
			.expect_err("fatal");

		assert!(error.to_string().contains("model not loaded"));
	}
}
