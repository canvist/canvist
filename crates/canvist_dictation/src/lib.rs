#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_dictation`
//!
//! The wire protocol between a canvist client and a dictation relay: one
//! WebSocket per dictation session at `GET /v1/dictation`.
//!
//! - The client sends [`ClientMessage::Start`] as JSON text, then audio as
//!   binary frames of 16-bit little-endian PCM, mono, at [`SAMPLE_RATE`],
//!   then [`ClientMessage::Stop`].
//! - The relay answers with [`RelayMessage`]s: `ready`, a `partial` draft
//!   for each segment as it changes, one `final` per segment in segment
//!   order, non-fatal or fatal `error`s, and `done`.
//!
//! The types hold no behaviour beyond encoding. Joining segment text to the
//! document is the editor's job, because it depends on the text before the
//! caret.
//!
//! ```
//! use canvist_dictation::ClientMessage;
//! use canvist_dictation::RelayMessage;
//!
//! let start = ClientMessage::start();
//! assert_eq!(start.to_json(), r#"{"type":"start","polish":true}"#);
//!
//! let message = RelayMessage::from_json(r#"{ "type": "final", "segment": 0, "text": "Hi." }"#)?;
//! assert_eq!(
//! 	message,
//! 	RelayMessage::Final {
//! 		segment: 0,
//! 		text: "Hi.".to_owned()
//! 	}
//! );
//! # Ok::<(), serde_json::Error>(())
//! ```

use serde::Deserialize;
use serde::Serialize;

/// The audio sample rate clients send, in hertz.
pub const SAMPLE_RATE: u32 = 16_000;
/// The number of audio channels clients send: mono.
pub const CHANNELS: u16 = 1;
/// Bytes in one audio sample: 16-bit little-endian PCM.
pub const BYTES_PER_SAMPLE: usize = 2;
/// The most characters of `context` a relay reads; it keeps the last ones.
pub const MAX_CONTEXT_CHARS: usize = 2_000;
/// The path a relay serves the protocol on.
pub const PATH: &str = "/v1/dictation";

/// A message from the client to the relay, sent as JSON text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
	/// Begin a session. It must be the first message.
	Start {
		/// A BCP 47 language tag. `None` lets the provider detect it.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		language: Option<String>,
		/// Up to [`MAX_CONTEXT_CHARS`] characters of text before the caret,
		/// used only to bias names and punctuation.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		context: Option<String>,
		/// Whether to run the polish pass when the relay has one configured.
		#[serde(default = "polish_by_default")]
		polish: bool,
	},
	/// No more audio is coming: finish every open segment, send `done`, and
	/// close.
	Stop,
}

fn polish_by_default() -> bool {
	true
}

impl ClientMessage {
	/// A `start` message with no language or context, and polish on.
	pub fn start() -> Self {
		Self::Start {
			language: None,
			context: None,
			polish: true,
		}
	}

	/// Encode as the JSON text the protocol sends.
	///
	/// It is built by hand, as [`RelayMessage::from_json`] decodes by hand,
	/// so native clients carry no tagged-enum serializer. A test holds it to
	/// the derived `Serialize`.
	#[must_use]
	pub fn to_json(&self) -> String {
		let Self::Start {
			language,
			context,
			polish,
		} = self
		else {
			return r#"{"type":"stop"}"#.to_owned();
		};
		let mut json = String::from(r#"{"type":"start""#);

		// Fields in the order the derive writes them, each value escaped
		// by serde_json.
		for (name, value) in [("language", language), ("context", context)] {
			if let Some(value) = value {
				json.push_str(",\"");
				json.push_str(name);
				json.push_str("\":");
				json.push_str(&serde_json::Value::from(value.as_str()).to_string());
			}
		}

		json.push_str(if *polish {
			r#","polish":true}"#
		} else {
			r#","polish":false}"#
		});
		json
	}

	/// Decode a JSON text message.
	pub fn from_json(text: &str) -> serde_json::Result<Self> {
		serde_json::from_str(text)
	}
}

/// A message from the relay to the client, sent as JSON text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RelayMessage {
	/// The relay reached the speech provider; audio is being transcribed.
	Ready,
	/// A draft of a segment. It replaces everything the client shows for the
	/// segment.
	Partial {
		/// The segment, numbered from 0 in each session.
		segment: u32,
		/// The draft text, with no leading or trailing space.
		text: String,
	},
	/// A segment's last text. Finals arrive in segment order, and a segment
	/// never changes after its final.
	Final {
		/// The segment, numbered from 0 in each session.
		segment: u32,
		/// The final text, with no leading or trailing space. It is empty when
		/// the segment held no words.
		text: String,
	},
	/// Something went wrong.
	Error {
		/// What kind of failure it was.
		code: ErrorCode,
		/// A human-readable explanation, for logs rather than end users.
		message: String,
		/// Whether the relay closes the socket next.
		fatal: bool,
	},
	/// Every segment is final; the relay closes the socket next.
	Done,
}

impl RelayMessage {
	/// An `error` message.
	pub fn error(code: ErrorCode, message: impl Into<String>, fatal: bool) -> Self {
		Self::Error {
			code,
			message: message.into(),
			fatal,
		}
	}

	/// Encode as the JSON text the protocol sends.
	pub fn to_json(&self) -> serde_json::Result<String> {
		serde_json::to_string(self)
	}

	/// Decode a JSON text message.
	///
	/// It is decoded by hand from a [`serde_json::Value`] rather than
	/// through the derived `Deserialize`. An internally tagged enum buffers
	/// every message through serde's generic content type, which costs every
	/// native client tens of kilobytes for five small messages. The derive
	/// still describes the same shape, and a test holds the two together.
	pub fn from_json(text: &str) -> serde_json::Result<Self> {
		let value: serde_json::Value = serde_json::from_str(text)?;
		let field = |name: &'static str| {
			value
				.get(name)
				.ok_or_else(|| invalid(&format!("missing field `{name}`")))
		};
		let text = |name| {
			field(name)?
				.as_str()
				.map(str::to_owned)
				.ok_or_else(|| invalid(&format!("`{name}` must be a string")))
		};
		let segment = || {
			field("segment")?
				.as_u64()
				.and_then(|segment| u32::try_from(segment).ok())
				.ok_or_else(|| invalid("`segment` must be a whole number"))
		};

		match value.get("type").and_then(serde_json::Value::as_str) {
			Some("ready") => Ok(Self::Ready),
			Some("done") => Ok(Self::Done),
			Some("partial") => {
				Ok(Self::Partial {
					segment: segment()?,
					text: text("text")?,
				})
			}
			Some("final") => {
				Ok(Self::Final {
					segment: segment()?,
					text: text("text")?,
				})
			}
			Some("error") => {
				Ok(Self::Error {
					code: ErrorCode::named(&text("code")?)
						.ok_or_else(|| invalid("unknown error `code`"))?,
					message: text("message")?,
					fatal: field("fatal")?
						.as_bool()
						.ok_or_else(|| invalid("`fatal` must be true or false"))?,
				})
			}
			_ => Err(invalid("unknown message `type`")),
		}
	}
}

/// A decoding error saying what was wrong with a message.
fn invalid(reason: &str) -> serde_json::Error {
	<serde_json::Error as serde::de::Error>::custom(reason)
}

/// What kind of failure an `error` message reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
	/// The token is missing, invalid, expired, or not for dictation.
	Unauthorized,
	/// The client sent something the protocol does not allow.
	BadRequest,
	/// The speech provider failed or could not be reached.
	Provider,
	/// The polish pass failed or timed out; the raw transcript was used.
	Polish,
	/// A session limit was reached: duration, idle time, or frame size.
	Limit,
	/// The relay failed.
	Internal,
}

impl ErrorCode {
	/// The code with its wire name, such as `bad_request`.
	#[must_use]
	pub fn named(name: &str) -> Option<Self> {
		match name {
			"unauthorized" => Some(Self::Unauthorized),
			"bad_request" => Some(Self::BadRequest),
			"provider" => Some(Self::Provider),
			"polish" => Some(Self::Polish),
			"limit" => Some(Self::Limit),
			"internal" => Some(Self::Internal),
			_ => None,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Remove the white space outside strings that the design doc uses for
	/// readability, giving the compact form the protocol sends.
	fn compact(json: &str) -> String {
		let mut compacted = String::with_capacity(json.len());
		let mut in_string = false;
		let mut escaped = false;

		for character in json.chars() {
			if !in_string && character.is_whitespace() {
				continue;
			}

			compacted.push(character);
			let quote = character == '"' && !escaped;
			escaped = in_string && !escaped && character == '\\';
			in_string ^= quote;
		}

		compacted
	}

	#[test]
	fn the_documented_start_message_round_trips() {
		let documented = r#"{
	"type": "start",
	"language": "en",
	"context": "The text before the caret, for punctuation and names.",
	"polish": true
}"#;
		let message = ClientMessage::from_json(documented).expect("decodes");

		assert_eq!(
			message,
			ClientMessage::Start {
				language: Some("en".to_owned()),
				context: Some("The text before the caret, for punctuation and names.".to_owned()),
				polish: true,
			}
		);
		assert_eq!(
			message.to_json(),
			r#"{"type":"start","language":"en","context":"The text before the caret, for punctuation and names.","polish":true}"#
		);
	}

	#[test]
	fn start_fields_are_optional_and_polish_defaults_on() {
		let message = ClientMessage::from_json(r#"{"type":"start"}"#).expect("decodes");
		assert_eq!(message, ClientMessage::start());

		let off = ClientMessage::from_json(r#"{"type":"start","polish":false}"#).expect("decodes");
		assert!(matches!(off, ClientMessage::Start { polish: false, .. }));
	}

	#[test]
	fn stop_round_trips() {
		let documented = r#"{ "type": "stop" }"#;
		let message = ClientMessage::from_json(documented).expect("decodes");

		assert_eq!(message, ClientMessage::Stop);
		assert_eq!(message.to_json(), compact(documented));
	}

	#[test]
	fn hand_encoding_agrees_with_the_derive() {
		let start = |language: Option<&str>, context: Option<&str>, polish| {
			ClientMessage::Start {
				language: language.map(str::to_owned),
				context: context.map(str::to_owned),
				polish,
			}
		};
		let messages = [
			ClientMessage::start(),
			ClientMessage::Stop,
			start(Some("en-GB"), None, false),
			start(None, Some("Quotes \" and\nlines"), true),
			start(Some("fr"), Some("Café"), true),
		];

		for message in messages {
			let derived = serde_json::to_string(&message).expect("encodes");
			assert_eq!(message.to_json(), derived);
		}
	}

	#[test]
	fn the_documented_relay_messages_round_trip() {
		let documented = [
			(r#"{ "type": "ready" }"#, RelayMessage::Ready),
			(
				r#"{ "type": "partial", "segment": 0, "text": "the meeting is on tuesday" }"#,
				RelayMessage::Partial {
					segment: 0,
					text: "the meeting is on tuesday".to_owned(),
				},
			),
			(
				r#"{ "type": "partial", "segment": 0, "text": "the meeting is on Tuesday no Wednesday" }"#,
				RelayMessage::Partial {
					segment: 0,
					text: "the meeting is on Tuesday no Wednesday".to_owned(),
				},
			),
			(
				r#"{ "type": "final", "segment": 0, "text": "The meeting is on Wednesday." }"#,
				RelayMessage::Final {
					segment: 0,
					text: "The meeting is on Wednesday.".to_owned(),
				},
			),
			(
				r#"{ "type": "error", "code": "provider", "message": "…", "fatal": false }"#,
				RelayMessage::error(ErrorCode::Provider, "…", false),
			),
			(r#"{ "type": "done" }"#, RelayMessage::Done),
		];

		for (json, expected) in documented {
			let message = RelayMessage::from_json(json).expect("decodes");
			assert_eq!(message, expected, "{json}");
			assert_eq!(message.to_json().expect("encodes"), compact(json), "{json}");
		}
	}

	#[test]
	fn every_error_code_has_its_documented_name() {
		let codes = [
			(ErrorCode::Unauthorized, "unauthorized"),
			(ErrorCode::BadRequest, "bad_request"),
			(ErrorCode::Provider, "provider"),
			(ErrorCode::Polish, "polish"),
			(ErrorCode::Limit, "limit"),
			(ErrorCode::Internal, "internal"),
		];

		for (code, name) in codes {
			let json = serde_json::to_string(&code).expect("encodes");
			assert_eq!(json, format!("\"{name}\""));
			assert_eq!(
				serde_json::from_str::<ErrorCode>(&json).expect("decodes"),
				code
			);
			assert_eq!(ErrorCode::named(name), Some(code));
		}

		assert_eq!(ErrorCode::named("Provider"), None);
	}

	#[test]
	fn hand_decoding_agrees_with_the_derive() {
		let messages = [
			r#"{"type":"ready"}"#,
			r#"{"type":"done"}"#,
			r#"{"type":"partial","segment":3,"text":"the cat sat"}"#,
			r#"{"type":"final","segment":3,"text":"The cat sat.","extra":1}"#,
			r#"{"type":"error","code":"polish","message":"timed out","fatal":false}"#,
			r#"{"type":"partial","segment":4294967296,"text":""}"#,
			r#"{"type":"partial","segment":1.5,"text":""}"#,
			r#"{"type":"partial","segment":1}"#,
			r#"{"type":"final","segment":1,"text":2}"#,
			r#"{"type":"error","code":"polish","message":""}"#,
			r#"{"type":"error","code":"polish","message":"","fatal":"no"}"#,
			r#"{"type":"error","code":"Polish","message":"","fatal":true}"#,
			r#"{"type":"pause"}"#,
			r#"{"segment":1,"text":""}"#,
			"[1,2]",
			"not json",
		];

		for message in messages {
			assert_eq!(
				RelayMessage::from_json(message).ok(),
				serde_json::from_str::<RelayMessage>(message).ok(),
				"{message}"
			);
		}
	}

	#[test]
	fn unknown_types_and_malformed_messages_are_refused() {
		assert!(ClientMessage::from_json(r#"{"type":"pause"}"#).is_err());
		assert!(ClientMessage::from_json(r#"{"language":"en"}"#).is_err());
		assert!(ClientMessage::from_json("start").is_err());
		assert!(RelayMessage::from_json(r#"{"type":"final","segment":-1,"text":""}"#).is_err());
		assert!(
			RelayMessage::from_json(r#"{"type":"error","code":"nope","message":"","fatal":true}"#)
				.is_err()
		);
	}

	#[test]
	fn the_audio_format_is_16_khz_mono_pcm16() {
		assert_eq!(SAMPLE_RATE, 16_000);
		assert_eq!(CHANNELS, 1);
		assert_eq!(BYTES_PER_SAMPLE, 2);
	}
}
