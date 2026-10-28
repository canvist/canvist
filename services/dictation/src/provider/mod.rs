//! Speech-to-text providers.
//!
//! Every provider is a WebSocket API, so each adapter is a state machine
//! with no I/O of its own: it turns audio, turn ends, and `stop` into the
//! messages its provider expects, and turns the provider's messages into
//! [`Event`]s numbered by segment. The session owns both sockets and the
//! clock, which keeps the adapters small and testable without a network.

mod deepgram;
mod openai;
mod vllm;

use std::fmt::Write;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::handshake::client::Request;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::http::header::AUTHORIZATION;

pub use self::deepgram::Deepgram;
pub use self::openai::OpenAi;
pub use self::vllm::Vllm;
use crate::config::ProviderConfig;
use crate::config::ProviderKind;
use crate::config::Secret;

/// The longest language tag accepted: BCP 47's recommended buffer size.
const MAX_LANGUAGE_TAG_BYTES: usize = 35;
/// The most of a provider's error message kept for the log.
const MAX_ERROR_CHARS: usize = 200;

/// Something a provider said about the transcript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
	/// A draft of `segment`, replacing its earlier drafts.
	Partial {
		/// The segment, numbered from 0.
		segment: u32,
		/// The draft, trimmed.
		text: String,
	},
	/// The raw final transcript of `segment`.
	Final {
		/// The segment, numbered from 0.
		segment: u32,
		/// The transcript, trimmed. Empty when the segment held no words.
		text: String,
	},
	/// A failure that loses part of the transcript but not the session.
	Warning(String),
}

/// What an adapter asks the session to do after each call.
#[derive(Debug, Default)]
pub struct Outbox {
	/// Messages for the provider, in order.
	pub messages: Vec<Message>,
	/// Events for the client, in order.
	pub events: Vec<Event>,
}

impl Outbox {
	/// Queue a JSON message for the provider.
	fn send(&mut self, value: &serde_json::Value) {
		self.messages.push(Message::text(value.to_string()));
	}

	fn partial(&mut self, segment: u32, text: &str) {
		self.events.push(Event::Partial {
			segment,
			text: text.trim().to_owned(),
		});
	}

	fn final_text(&mut self, segment: u32, text: &str) {
		self.events.push(Event::Final {
			segment,
			text: text.trim().to_owned(),
		});
	}
}

/// A provider failure that ends the session.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
	/// The provider could not be reached.
	#[error("could not reach the provider: {0}")]
	Connect(String),
	/// The provider refused the WebSocket handshake.
	#[error("the provider refused the connection with HTTP {0}")]
	Refused(u16),
	/// The provider sent something that is not its protocol.
	#[error("the provider sent an unreadable message")]
	Malformed,
	/// The provider reported an error.
	#[error("the provider reported an error: {0}")]
	Remote(String),
	/// The provider closed the connection before finishing.
	#[error("the provider closed the connection")]
	Closed,
	/// The configured URL or key cannot form a request.
	#[error("the provider is misconfigured: {0}")]
	Config(&'static str),
}

/// One provider's protocol, for one session.
pub trait Adapter: Send {
	/// The WebSocket handshake request: URL and headers.
	fn request(&self) -> Result<Request, ProviderError>;

	/// The connection opened.
	fn open(&mut self, out: &mut Outbox);

	/// The client sent `samples` of 16 kHz audio.
	fn audio(&mut self, samples: &[i16], out: &mut Outbox);

	/// Voice activity detection heard the end of a turn at `now`.
	fn end_of_turn(&mut self, now: Instant, out: &mut Outbox);

	/// The client sent `stop`: no more audio is coming.
	fn finish(&mut self, out: &mut Outbox);

	/// The provider sent a text message.
	fn receive(&mut self, text: &str, out: &mut Outbox) -> Result<(), ProviderError>;

	/// When the adapter next needs [`Adapter::tick`], if ever.
	fn deadline(&self) -> Option<Instant> {
		None
	}

	/// The deadline passed.
	fn tick(&mut self, _now: Instant, _out: &mut Outbox) {}

	/// Whether, after `stop`, every segment has its final.
	fn is_finished(&self) -> bool;

	/// The provider closed the connection. Closing is a failure unless the
	/// adapter has finished.
	fn closed(&mut self) -> Result<(), ProviderError> {
		if self.is_finished() {
			Ok(())
		} else {
			Err(ProviderError::Closed)
		}
	}
}

/// What a session asks of its provider.
#[derive(Clone, Debug, Default)]
pub struct SessionOptions {
	/// A validated BCP 47 language tag, or `None` to detect it.
	pub language: Option<String>,
}

/// The adapter for `config`'s provider, for one session.
pub fn adapter(config: &ProviderConfig, options: &SessionOptions) -> Box<dyn Adapter> {
	match config.kind {
		ProviderKind::OpenAi => Box::new(OpenAi::new(config, options)),
		ProviderKind::Deepgram => Box::new(Deepgram::new(config, options)),
		ProviderKind::Vllm => Box::new(Vllm::new(config)),
	}
}

/// Whether `tag` looks like a BCP 47 language tag. Tags reach provider URLs
/// and JSON, so only letters, digits, and inner hyphens pass.
pub fn is_language_tag(tag: &str) -> bool {
	(1..=MAX_LANGUAGE_TAG_BYTES).contains(&tag.len())
		&& !tag.starts_with('-')
		&& !tag.ends_with('-')
		&& tag
			.bytes()
			.all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

/// A handshake request for `url` with an `Authorization` header of
/// `scheme` and `key`, when there is a key.
fn request(url: &str, scheme: &str, key: Option<&Secret>) -> Result<Request, ProviderError> {
	let mut request = url
		.into_client_request()
		.map_err(|_| ProviderError::Config("the URL is invalid"))?;

	if let Some(key) = key {
		let mut value = HeaderValue::from_str(&format!("{scheme} {}", key.expose()))
			.map_err(|_| ProviderError::Config("the API key has invalid characters"))?;
		value.set_sensitive(true);
		request.headers_mut().insert(AUTHORIZATION, value);
	}

	Ok(request)
}

/// Base64 of `samples` as little-endian PCM16, as Realtime APIs carry audio.
fn base64_pcm16(samples: &[i16]) -> String {
	STANDARD.encode(crate::audio::encode_pcm16(samples))
}

/// Percent-encode `value` for a URL query.
fn query_value(value: &str) -> String {
	let mut encoded = String::with_capacity(value.len());

	for byte in value.bytes() {
		if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
			encoded.push(char::from(byte));
		} else {
			let _ = write!(encoded, "%{byte:02X}");
		}
	}

	encoded
}

/// The message in a provider's `error` event, which providers shape as a
/// string, `{ "message": … }`, or a top-level `message`.
fn error_message(event: &serde_json::Value) -> String {
	let error = &event["error"];
	let message = error
		.as_str()
		.or_else(|| error["message"].as_str())
		.or_else(|| event["message"].as_str())
		.or_else(|| event["description"].as_str())
		.unwrap_or("unknown error");

	message.chars().take(MAX_ERROR_CHARS).collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn language_tags_are_checked() {
		assert!(is_language_tag("en"));
		assert!(is_language_tag("zh-Hant-TW"));
		assert!(!is_language_tag(""));
		assert!(!is_language_tag("-en"));
		assert!(!is_language_tag("en&x=1"));
		assert!(!is_language_tag("en us"));
	}

	#[test]
	fn query_values_are_percent_encoded() {
		assert_eq!(query_value("nova-3"), "nova-3");
		assert_eq!(query_value("a b&c/d"), "a%20b%26c%2Fd");
	}

	#[test]
	fn keys_are_sent_as_sensitive_headers() {
		let request =
			request("wss://example.com/v1", "Token", Some(&Secret::new("abc"))).expect("valid");
		let value = &request.headers()[AUTHORIZATION];

		assert_eq!(value, "Token abc");
		assert!(value.is_sensitive());
		assert!(super::request("wss://example.com", "Bearer", Some(&Secret::new("a\nb"))).is_err());
	}

	#[test]
	fn error_messages_come_from_every_shape() {
		let shapes = [
			serde_json::json!({ "error": "plain" }),
			serde_json::json!({ "error": { "message": "nested" } }),
			serde_json::json!({ "message": "top" }),
			serde_json::json!({ "description": "deepgram" }),
		];
		let messages: Vec<String> = shapes.iter().map(error_message).collect();

		assert_eq!(messages, ["plain", "nested", "top", "deepgram"]);
		assert_eq!(error_message(&serde_json::json!({})), "unknown error");
	}
}
