//! Dictation: speech becoming text as someone talks.
//!
//! A speech source (canvist's dictation relay, or the platform's own
//! recognizer) reports each segment of speech, first as provisional text
//! and then as final text. Provisional text shows faint at the caret but is
//! kept out of the document, so it is never shared, saved, or undone. Final
//! text is inserted in segment order, each segment as one undo step, and
//! fades into ink while frames are animated.

use canvist_dictation::ClientMessage;
use canvist_dictation::ErrorCode;
use canvist_dictation::MAX_CONTEXT_CHARS;
use canvist_dictation::RelayMessage;
use canvist_headless::canvist_core::Transcript;

use crate::CanvistError;
use crate::Editor;
use crate::OffsetRange;
use crate::error::Result;

/// What canvist's dictation relay said, once the editor applied it.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum DictationEvent {
	/// The relay reached its speech provider: audio is being transcribed.
	Ready,
	/// A segment's draft (`is_final` false) or final text, already shown or
	/// inserted.
	Transcript {
		/// The segment, numbered from 0 in each session.
		segment: u32,
		/// Whether the text was final and is now in the document.
		is_final: bool,
	},
	/// Something went wrong. A fatal error ends the session: the relay closes
	/// the socket next, and the editor has dropped the draft.
	Error {
		/// What kind of failure it was.
		code: DictationErrorCode,
		/// An explanation for logs, not for people using the app.
		message: String,
		/// Whether the relay closes the socket next.
		fatal: bool,
	},
	/// Every segment is final and the session is over: the relay closes the
	/// socket next.
	Done,
}

/// What kind of failure a dictation relay reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum DictationErrorCode {
	/// The token was missing, invalid, or expired.
	Unauthorized,
	/// The client sent something the protocol doesn't allow.
	BadRequest,
	/// The speech provider failed.
	Provider,
	/// The polish model failed, so the raw transcript was used.
	Polish,
	/// The session hit a limit, such as its longest duration.
	Limit,
	/// The relay failed in a way it couldn't classify.
	Internal,
}

impl From<ErrorCode> for DictationErrorCode {
	fn from(code: ErrorCode) -> Self {
		match code {
			ErrorCode::Unauthorized => Self::Unauthorized,
			ErrorCode::BadRequest => Self::BadRequest,
			ErrorCode::Provider => Self::Provider,
			ErrorCode::Polish => Self::Polish,
			ErrorCode::Limit => Self::Limit,
			ErrorCode::Internal => Self::Internal,
		}
	}
}

/// The message that ends a relay session: no more audio is coming.
#[uniffi::export]
#[must_use]
pub fn dictation_stop_message() -> String {
	ClientMessage::Stop.to_json()
}

#[uniffi::export]
impl Editor {
	/// Report what a speech source said about `segment`, numbered from 0 in
	/// each dictation session. Provisional text (`is_final` false) replaces
	/// whatever the segment showed; final text never changes again and is
	/// inserted once every earlier segment is final. The first report
	/// replaces a selection, as typing does. Returns whether the document or
	/// the provisional text changed.
	pub fn dictate(&self, segment: u32, text: &str, is_final: bool) -> Result<bool> {
		let transcript = if is_final {
			Transcript::Final(text)
		} else {
			Transcript::Partial(text)
		};

		self.change(|session| {
			let before = session.draft();
			let edited = session.edit(|editor, env| editor.dictate(segment, transcript, env))?;
			Ok(edited || session.draft() != before)
		})
	}

	/// Apply a text message from canvist's dictation relay: show a draft,
	/// insert final text, or end the session. Binary frames never come from
	/// the relay. Throws `invalidArgs` for a message that is not the relay's
	/// JSON.
	pub fn receive_dictation(&self, message: &str) -> Result<DictationEvent> {
		let message = RelayMessage::from_json(message)
			.map_err(|error| CanvistError::invalid_args("receive_dictation", error))?;

		match message {
			RelayMessage::Ready => Ok(DictationEvent::Ready),
			RelayMessage::Partial { segment, text } => {
				self.dictate(segment, &text, false)?;
				Ok(DictationEvent::Transcript {
					segment,
					is_final: false,
				})
			}
			RelayMessage::Final { segment, text } => {
				self.dictate(segment, &text, true)?;
				Ok(DictationEvent::Transcript {
					segment,
					is_final: true,
				})
			}
			RelayMessage::Error {
				code,
				message,
				fatal,
			} => {
				if fatal {
					self.stop_dictation();
				}

				Ok(DictationEvent::Error {
					code: code.into(),
					message,
					fatal,
				})
			}
			RelayMessage::Done => {
				self.stop_dictation();
				Ok(DictationEvent::Done)
			}
		}
	}

	/// The message that opens a relay session. It carries the text before
	/// the caret, so the relay gets names and punctuation right; `language`
	/// is a BCP 47 tag, or `None` for the provider to detect it; `polish`
	/// asks for the relay's language-model cleanup when it has one.
	pub fn dictation_start_message(&self, language: Option<String>, polish: bool) -> String {
		let context = self.text_before_caret();

		ClientMessage::Start {
			language,
			context: (!context.is_empty()).then_some(context),
			polish,
		}
		.to_json()
	}

	/// Up to [`MAX_CONTEXT_CHARS`] characters of the text before the caret.
	fn text_before_caret(&self) -> String {
		let session = self.lock();
		let head = session.selection().head;
		let window = (MAX_CONTEXT_CHARS * 2) as u32;
		let range = OffsetRange::between(head.saturating_sub(window), head);
		let text = session.text.text(session.doc.document(), range);
		let skip = text.chars().count().saturating_sub(MAX_CONTEXT_CHARS);

		text.chars().skip(skip).collect()
	}

	/// Stop dictating: forget every segment that is not final, so the next
	/// session numbers segments from 0 again. Returns whether provisional
	/// text was showing.
	pub fn stop_dictation(&self) -> bool {
		self.change(super::session::Session::stop_dictation)
	}

	/// The provisional text showing at the caret, if any, as it reads with
	/// the spaces that join it to the text around it.
	pub fn dictation_draft(&self) -> Option<String> {
		self.lock().draft()
	}

	/// Animate provisional, settling, and kinetic text. Turn it on once the
	/// view asks for a frame on every display refresh while
	/// [`Frame::animated`](crate::Frame::animated) is true, and leave it off
	/// for people who prefer reduced motion: provisional text then shows at a
	/// steady faint opacity, and final text in full ink at once. Off by
	/// default.
	pub fn set_animations(&self, enabled: bool) {
		self.lock().set_animations(enabled);
	}
}
