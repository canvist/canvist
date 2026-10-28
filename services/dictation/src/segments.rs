//! The order segments reach the client in.
//!
//! Drafts go out as soon as they arrive. Finals go out in segment order: a
//! provider may complete segment 2 before segment 1, and polishing takes a
//! different time for each segment, so a final waits here until every
//! earlier segment's final is sent. Once a segment is final, or its final is
//! waiting or being polished, later drafts of it are dropped.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use canvist_dictation::MAX_CONTEXT_CHARS;
use canvist_dictation::RelayMessage;

/// Segments of one session, between the provider and the client.
#[derive(Debug, Default)]
pub struct Segments {
	/// The segment whose final goes out next.
	next_final: u32,
	/// Finals waiting for an earlier segment's.
	ready: BTreeMap<u32, String>,
	/// Segments whose final is being polished.
	polishing: BTreeSet<u32>,
	/// The latest text of every segment that is not final: its draft, or its
	/// raw transcript while it is polished.
	drafts: BTreeMap<u32, String>,
	/// The tail of the finals sent so far, for polish context.
	written: String,
}

impl Segments {
	/// A draft of `segment`, unless the segment is already closed.
	pub fn partial(&mut self, segment: u32, text: String) -> Option<RelayMessage> {
		if self.is_closed(segment) {
			return None;
		}

		self.drafts.insert(segment, text.clone());
		Some(RelayMessage::Partial { segment, text })
	}

	/// Start polishing `segment`'s raw transcript. Returns the draft that
	/// shows the raw transcript meanwhile, or `None` when the segment is
	/// already closed and must not be polished.
	pub fn polishing(&mut self, segment: u32, raw: String) -> Option<RelayMessage> {
		let draft = self.partial(segment, raw)?;
		self.polishing.insert(segment);
		Some(draft)
	}

	/// `segment`'s final text. Returns the finals that can now go out, in
	/// order.
	pub fn finish(&mut self, segment: u32, text: String) -> Vec<RelayMessage> {
		if segment < self.next_final || self.ready.contains_key(&segment) {
			return Vec::new();
		}

		self.polishing.remove(&segment);
		self.drafts.remove(&segment);
		self.ready.insert(segment, text);
		self.release()
	}

	/// Close every segment the provider left open: each takes its latest
	/// draft as its final, or nothing when it has none. Used when the
	/// provider runs out of time, so the client is never left with drafts.
	pub fn force_close(&mut self) -> Vec<RelayMessage> {
		let last = self
			.drafts
			.keys()
			.chain(self.ready.keys())
			.chain(self.polishing.iter())
			.max()
			.copied();
		self.polishing.clear();

		let Some(last) = last else {
			return Vec::new();
		};

		for segment in self.next_final..=last {
			let draft = self.drafts.remove(&segment).unwrap_or_default();
			self.ready.entry(segment).or_insert(draft);
		}

		self.release()
	}

	/// Whether every segment the client has seen is final.
	pub fn is_settled(&self) -> bool {
		self.ready.is_empty() && self.polishing.is_empty() && self.drafts.is_empty()
	}

	/// How many finals went out.
	pub fn finals_sent(&self) -> u32 {
		self.next_final
	}

	/// What precedes `segment` in the document, for polishing it: the text
	/// before the caret, then every earlier segment's best text, keeping the
	/// last [`MAX_CONTEXT_CHARS`] characters.
	pub fn context_before(&self, segment: u32, before_caret: &str) -> String {
		let mut context = String::from(before_caret);
		let pending = self
			.ready
			.range(..segment)
			.chain(self.drafts.range(..segment))
			.collect::<BTreeMap<_, _>>();

		for text in std::iter::once(&self.written).chain(pending.into_values()) {
			push_text(&mut context, text);
		}

		tail(&context, MAX_CONTEXT_CHARS).to_owned()
	}

	fn is_closed(&self, segment: u32) -> bool {
		segment < self.next_final
			|| self.ready.contains_key(&segment)
			|| self.polishing.contains(&segment)
	}

	/// Send every final whose earlier segments are all final.
	fn release(&mut self) -> Vec<RelayMessage> {
		let mut finals = Vec::new();

		while let Some(text) = self.ready.remove(&self.next_final) {
			push_text(&mut self.written, &text);
			self.written = tail(&self.written, MAX_CONTEXT_CHARS).to_owned();
			finals.push(RelayMessage::Final {
				segment: self.next_final,
				text,
			});
			self.next_final += 1;
		}

		finals
	}
}

/// Append `text` to `buffer` with one space between them.
fn push_text(buffer: &mut String, text: &str) {
	if text.is_empty() {
		return;
	}

	if !buffer.is_empty() && !buffer.ends_with(char::is_whitespace) {
		buffer.push(' ');
	}

	buffer.push_str(text);
}

/// The last `count` characters of `text`.
pub fn tail(text: &str, count: usize) -> &str {
	let skip = text.chars().count().saturating_sub(count);
	let start = text
		.char_indices()
		.nth(skip)
		.map_or(text.len(), |(index, _)| index);

	&text[start..]
}

#[cfg(test)]
mod tests {
	use super::*;

	fn final_message(segment: u32, text: &str) -> RelayMessage {
		RelayMessage::Final {
			segment,
			text: text.to_owned(),
		}
	}

	#[test]
	fn finals_wait_for_earlier_segments() {
		let mut segments = Segments::default();

		assert!(segments.finish(1, "second".to_owned()).is_empty());
		assert!(segments.finish(2, "third".to_owned()).is_empty());
		assert_eq!(
			segments.finish(0, "first".to_owned()),
			[
				final_message(0, "first"),
				final_message(1, "second"),
				final_message(2, "third"),
			]
		);
		assert!(segments.is_settled());
		assert_eq!(segments.finals_sent(), 3);
	}

	#[test]
	fn closed_segments_take_no_more_drafts_or_finals() {
		let mut segments = Segments::default();
		assert!(segments.partial(0, "draft".to_owned()).is_some());
		segments.finish(0, "Final.".to_owned());

		assert!(segments.partial(0, "late".to_owned()).is_none());
		assert!(segments.finish(0, "again".to_owned()).is_empty());

		segments.finish(2, "waiting".to_owned());
		assert!(segments.partial(2, "late".to_owned()).is_none());
	}

	#[test]
	fn polishing_shows_the_raw_transcript_and_holds_later_finals() {
		let mut segments = Segments::default();

		assert_eq!(
			segments.polishing(0, "um the meeting".to_owned()),
			Some(RelayMessage::Partial {
				segment: 0,
				text: "um the meeting".to_owned()
			})
		);
		assert!(segments.partial(0, "late draft".to_owned()).is_none());
		assert!(segments.finish(1, "Next.".to_owned()).is_empty());
		assert!(!segments.is_settled());

		assert_eq!(
			segments.finish(0, "The meeting.".to_owned()),
			[final_message(0, "The meeting."), final_message(1, "Next."),]
		);
	}

	#[test]
	fn force_close_finalizes_drafts_and_fills_gaps() {
		let mut segments = Segments::default();
		segments.partial(0, "half a".to_owned());
		segments.polishing(1, "raw".to_owned());
		segments.finish(3, "Done.".to_owned());

		assert_eq!(
			segments.force_close(),
			[
				final_message(0, "half a"),
				final_message(1, "raw"),
				final_message(2, ""),
				final_message(3, "Done."),
			]
		);
		assert!(segments.is_settled());
		assert!(segments.force_close().is_empty());
	}

	#[test]
	fn context_joins_the_caret_text_and_earlier_segments() {
		let mut segments = Segments::default();
		segments.finish(0, "First.".to_owned());
		segments.partial(1, "second".to_owned());
		segments.partial(3, "fourth".to_owned());

		assert_eq!(segments.context_before(2, "Before"), "Before First. second");
		assert_eq!(segments.context_before(1, "Before "), "Before First.");

		let long = "x".repeat(MAX_CONTEXT_CHARS + 10);
		assert_eq!(
			segments.context_before(0, &long).chars().count(),
			MAX_CONTEXT_CHARS
		);
	}

	#[test]
	fn tails_respect_character_boundaries() {
		assert_eq!(tail("héllo", 4), "éllo");
		assert_eq!(tail("hi", 5), "hi");
		assert_eq!(tail("hi", 0), "");
	}
}
