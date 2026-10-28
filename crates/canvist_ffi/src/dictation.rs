//! Dictation: speech becoming text as someone talks.
//!
//! A speech source (canvist's dictation relay, or the platform's own
//! recognizer) reports each segment of speech, first as provisional text
//! and then as final text. Provisional text shows faint at the caret but is
//! kept out of the document, so it is never shared, saved, or undone. Final
//! text is inserted in segment order, each segment as one undo step, and
//! fades into ink while frames are animated.

use canvist_headless::canvist_core::Transcript;

use crate::Editor;
use crate::error::Result;

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
