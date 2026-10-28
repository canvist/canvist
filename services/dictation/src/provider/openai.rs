//! OpenAI's GA Realtime transcription API.
//!
//! The relay opens a `transcription` session with server turn detection
//! off, streams 24 kHz audio, and commits the buffer when its own voice
//! activity detection hears the end of a turn. Each commit becomes a
//! conversation item; its deltas are drafts and its `completed` event is the
//! final. Completions can arrive out of order, so items are numbered as
//! segments in commit order, which is the order the speaker said them.

use std::collections::HashMap;

use serde_json::json;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::handshake::client::Request;

use super::Adapter;
use super::Event;
use super::Outbox;
use super::ProviderError;
use super::SessionOptions;
use super::base64_pcm16;
use super::error_message;
use super::request;
use crate::audio::Resampler;
use crate::config::ProviderConfig;
use crate::config::Secret;

/// The rate the Realtime API takes PCM16 at.
const PROVIDER_RATE: u32 = 24_000;
/// The least audio the API commits: 100 ms at 16 kHz. Shorter buffers are
/// refused, so they wait for the next turn.
const MIN_COMMIT_SAMPLES: usize = 1_600;
/// The error the API sends for a commit with too little audio; the buffer is
/// kept, so it is not a failure.
const COMMIT_EMPTY: &str = "input_audio_buffer_commit_empty";

/// An OpenAI Realtime transcription session.
#[derive(Debug)]
pub struct OpenAi {
	url: String,
	model: String,
	key: Option<Secret>,
	language: Option<String>,
	resampler: Resampler,
	resampled: Vec<i16>,
	/// Client samples appended since the last commit.
	uncommitted: usize,
	/// Commits not yet acknowledged with `input_audio_buffer.committed`.
	pending_commits: usize,
	next_segment: u32,
	/// Items that have no final yet, by item id.
	items: HashMap<String, Item>,
	finishing: bool,
}

/// One committed turn's transcription.
#[derive(Debug, Default)]
struct Item {
	/// Its segment, once its commit is acknowledged.
	segment: Option<u32>,
	/// The deltas so far.
	text: String,
	/// The completed transcript, when it arrived before the commit's
	/// acknowledgement.
	completed: Option<String>,
}

impl OpenAi {
	/// A session for `config`, transcribing `options.language` when set.
	pub fn new(config: &ProviderConfig, options: &SessionOptions) -> Self {
		// The API takes an ISO 639-1 code: the primary subtag of the tag.
		let language = options
			.language
			.as_ref()
			.and_then(|tag| tag.split('-').next().map(str::to_ascii_lowercase));

		Self {
			url: format!("{}/v1/realtime?intent=transcription", config.url),
			model: config.model.clone(),
			key: config.key.clone(),
			language,
			resampler: Resampler::new(canvist_dictation::SAMPLE_RATE, PROVIDER_RATE),
			resampled: Vec::new(),
			uncommitted: 0,
			pending_commits: 0,
			next_segment: 0,
			items: HashMap::new(),
			finishing: false,
		}
	}

	fn commit(&mut self, out: &mut Outbox) {
		if self.uncommitted < MIN_COMMIT_SAMPLES {
			return;
		}

		out.send(&json!({ "type": "input_audio_buffer.commit" }));
		self.uncommitted = 0;
		self.pending_commits += 1;
	}

	fn committed(&mut self, item_id: &str, out: &mut Outbox) {
		self.pending_commits = self.pending_commits.saturating_sub(1);
		let segment = self.next_segment;
		self.next_segment += 1;

		let item = self.items.entry(item_id.to_owned()).or_default();
		item.segment = Some(segment);

		if let Some(transcript) = item.completed.take() {
			self.items.remove(item_id);
			out.final_text(segment, &transcript);
		} else if !item.text.trim().is_empty() {
			out.partial(segment, &item.text);
		}
	}

	fn delta(&mut self, item_id: String, delta: &str, out: &mut Outbox) {
		let item = self.items.entry(item_id).or_default();
		item.text.push_str(delta);

		if let Some(segment) = item.segment
			&& !item.text.trim().is_empty()
		{
			out.partial(segment, &item.text);
		}
	}

	fn completed(&mut self, item_id: &str, transcript: String, out: &mut Outbox) {
		let item = self.items.entry(item_id.to_owned()).or_default();

		let Some(segment) = item.segment else {
			item.completed = Some(transcript);

			return;
		};

		self.items.remove(item_id);
		out.final_text(segment, &transcript);
	}
}

impl Adapter for OpenAi {
	fn request(&self) -> Result<Request, ProviderError> {
		request(&self.url, "Bearer", self.key.as_ref())
	}

	fn open(&mut self, out: &mut Outbox) {
		let mut transcription = json!({ "model": self.model });

		if let Some(language) = &self.language {
			transcription["language"] = json!(language);
		}

		out.send(&json!({
			"type": "session.update",
			"session": {
				"type": "transcription",
				"audio": {
					"input": {
						"format": { "type": "audio/pcm", "rate": PROVIDER_RATE },
						"transcription": transcription,
						"turn_detection": null,
					},
				},
			},
		}));
	}

	fn audio(&mut self, samples: &[i16], out: &mut Outbox) {
		self.resampled.clear();
		self.resampler.process(samples, &mut self.resampled);

		if self.resampled.is_empty() {
			return;
		}

		out.send(&json!({
			"type": "input_audio_buffer.append",
			"audio": base64_pcm16(&self.resampled),
		}));
		self.uncommitted += samples.len();
	}

	fn end_of_turn(&mut self, _now: Instant, out: &mut Outbox) {
		self.commit(out);
	}

	fn finish(&mut self, out: &mut Outbox) {
		self.commit(out);
		self.finishing = true;
	}

	fn receive(&mut self, text: &str, out: &mut Outbox) -> Result<(), ProviderError> {
		let event: serde_json::Value =
			serde_json::from_str(text).map_err(|_| ProviderError::Malformed)?;
		let item_id = event["item_id"].as_str().unwrap_or_default().to_owned();
		let text_field = |name: &str| event[name].as_str().unwrap_or_default().to_owned();

		match event["type"].as_str().unwrap_or_default() {
			"input_audio_buffer.committed" => self.committed(&item_id, out),
			"conversation.item.input_audio_transcription.delta" => {
				self.delta(item_id, &text_field("delta"), out);
			}
			"conversation.item.input_audio_transcription.completed" => {
				self.completed(&item_id, text_field("transcript"), out);
			}
			"conversation.item.input_audio_transcription.failed" => {
				// Keep what was heard, so the segment still closes in order.
				let heard = self
					.items
					.get(&item_id)
					.map(|item| item.text.clone())
					.unwrap_or_default();
				out.events.push(Event::Warning(format!(
					"a segment failed to transcribe: {}",
					error_message(&event)
				)));
				self.completed(&item_id, heard, out);
			}
			"error" if event["error"]["code"] == COMMIT_EMPTY => {
				self.pending_commits = self.pending_commits.saturating_sub(1);
			}
			"error" => return Err(ProviderError::Remote(error_message(&event))),
			_ => {}
		}

		Ok(())
	}

	fn is_finished(&self) -> bool {
		self.finishing && self.pending_commits == 0 && self.items.is_empty()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::config::ProviderKind;

	fn adapter() -> OpenAi {
		let config = ProviderConfig::new(ProviderKind::OpenAi, Some(Secret::new("sk-test")));
		let options = SessionOptions {
			language: Some("en-GB".to_owned()),
		};
		OpenAi::new(&config, &options)
	}

	fn sent(out: &Outbox) -> Vec<serde_json::Value> {
		out.messages
			.iter()
			.map(|message| serde_json::from_str(message.to_text().expect("text")).expect("JSON"))
			.collect()
	}

	fn receive(adapter: &mut OpenAi, event: &serde_json::Value) -> Vec<Event> {
		let mut out = Outbox::default();
		adapter
			.receive(&event.to_string(), &mut out)
			.expect("accepted");
		out.events
	}

	#[test]
	fn the_session_is_a_24_khz_transcription_without_turn_detection() {
		let mut adapter = adapter();
		let mut out = Outbox::default();
		adapter.open(&mut out);

		let request = adapter.request().expect("valid");
		assert_eq!(
			request.uri().to_string(),
			"wss://api.openai.com/v1/realtime?intent=transcription"
		);
		assert_eq!(request.headers()["authorization"], "Bearer sk-test");

		let update = &sent(&out)[0];
		let input = &update["session"]["audio"]["input"];
		assert_eq!(update["session"]["type"], "transcription");
		assert_eq!(
			input["format"],
			json!({ "type": "audio/pcm", "rate": 24_000 })
		);
		assert_eq!(input["transcription"]["model"], "gpt-live-transcribe");
		assert_eq!(input["transcription"]["language"], "en");
		assert!(input["turn_detection"].is_null());
	}

	#[test]
	fn audio_is_appended_at_24_khz_and_short_buffers_are_not_committed() {
		let mut adapter = adapter();
		let mut out = Outbox::default();
		adapter.audio(&[100; 800], &mut out);
		adapter.end_of_turn(Instant::now(), &mut out);

		let messages = sent(&out);
		assert_eq!(messages.len(), 1, "50 ms is too short to commit");
		assert_eq!(messages[0]["type"], "input_audio_buffer.append");

		let audio = base64::Engine::decode(
			&base64::engine::general_purpose::STANDARD,
			messages[0]["audio"].as_str().expect("audio"),
		)
		.expect("base64");
		assert_eq!(
			audio.len() / 2,
			1_199,
			"800 samples at 16 kHz resample to 1,200"
		);

		let mut out = Outbox::default();
		adapter.audio(&[100; 800], &mut out);
		adapter.end_of_turn(Instant::now(), &mut out);
		assert_eq!(sent(&out)[1]["type"], "input_audio_buffer.commit");
	}

	#[test]
	fn segments_follow_commit_order_when_completions_do_not() {
		let mut adapter = adapter();
		receive(
			&mut adapter,
			&json!({ "type": "input_audio_buffer.committed", "item_id": "a" }),
		);
		receive(
			&mut adapter,
			&json!({ "type": "input_audio_buffer.committed", "item_id": "b" }),
		);

		let draft = receive(
			&mut adapter,
			&json!({ "type": "conversation.item.input_audio_transcription.delta", "item_id": "a", "delta": "hello " }),
		);
		assert_eq!(
			draft,
			[Event::Partial {
				segment: 0,
				text: "hello".to_owned()
			}]
		);

		let second = receive(
			&mut adapter,
			&json!({ "type": "conversation.item.input_audio_transcription.completed", "item_id": "b", "transcript": " world." }),
		);
		assert_eq!(
			second,
			[Event::Final {
				segment: 1,
				text: "world.".to_owned()
			}]
		);

		let first = receive(
			&mut adapter,
			&json!({ "type": "conversation.item.input_audio_transcription.completed", "item_id": "a", "transcript": "Hello." }),
		);
		assert_eq!(
			first,
			[Event::Final {
				segment: 0,
				text: "Hello.".to_owned()
			}]
		);
	}

	#[test]
	fn a_completion_before_its_commit_waits_for_its_number() {
		let mut adapter = adapter();
		let early = receive(
			&mut adapter,
			&json!({ "type": "conversation.item.input_audio_transcription.completed", "item_id": "a", "transcript": "Hi." }),
		);
		assert!(early.is_empty());

		let committed = receive(
			&mut adapter,
			&json!({ "type": "input_audio_buffer.committed", "item_id": "a" }),
		);
		assert_eq!(
			committed,
			[Event::Final {
				segment: 0,
				text: "Hi.".to_owned()
			}]
		);
	}

	#[test]
	fn finishing_waits_for_every_commit_and_completion() {
		let mut adapter = adapter();
		let mut out = Outbox::default();
		adapter.audio(&[100; 3_200], &mut out);
		adapter.finish(&mut out);
		assert!(!adapter.is_finished(), "the commit is unacknowledged");

		receive(
			&mut adapter,
			&json!({ "type": "input_audio_buffer.committed", "item_id": "a" }),
		);
		assert!(!adapter.is_finished(), "the item has no transcript");

		let failed = receive(
			&mut adapter,
			&json!({ "type": "conversation.item.input_audio_transcription.failed", "item_id": "a", "error": { "message": "boom" } }),
		);
		assert!(matches!(&failed[0], Event::Warning(message) if message.contains("boom")));
		assert_eq!(
			failed[1],
			Event::Final {
				segment: 0,
				text: String::new()
			}
		);
		assert!(adapter.is_finished());
		assert!(adapter.closed().is_ok());
	}

	#[test]
	fn errors_end_the_session_except_an_empty_commit() {
		let mut adapter = adapter();
		let mut out = Outbox::default();
		let empty =
			json!({ "type": "error", "error": { "code": COMMIT_EMPTY, "message": "too short" } });
		assert!(adapter.receive(&empty.to_string(), &mut out).is_ok());

		let fatal = json!({ "type": "error", "error": { "code": "invalid_model", "message": "no such model" } });
		let error = adapter
			.receive(&fatal.to_string(), &mut out)
			.expect_err("fatal");
		assert!(error.to_string().contains("no such model"));
		assert!(matches!(
			adapter.receive("not json", &mut out),
			Err(ProviderError::Malformed)
		));
	}
}
