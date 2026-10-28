//! Dictation: speech becoming text, one segment at a time.
//!
//! A speech model reports each segment of what someone says, first as
//! provisional text that may still change, then as final text that never
//! will. Provisional text is never part of the document: hosts lay it out at
//! the caret from [`Dictation::provisional`] and paint it faint, so it is not
//! shared with collaborators, saved, exported, or recorded in history.
//!
//! Final segments are inserted at the caret in segment order, each as its own
//! undo step, and [`Dictation::settling`] remembers where and when, so hosts
//! can fade the new text into ink. A selection is replaced as soon as the
//! first transcript arrives, as the first keystroke replaces it when typing.

use std::time::Duration;

use canvist_model::Bias;
use canvist_model::BlockId;
use canvist_model::Commit;
use canvist_model::Document;
use canvist_model::InlineText;
use canvist_model::Point;
use canvist_model::Selection;

use crate::CommandResult;
use crate::EditKind;
use crate::EditorState;
use crate::EditorTransaction;
use crate::commands;
use crate::commands::insert_at;
use crate::position;
use crate::structure;

/// How long [`Dictation::settling`] remembers inserted text: longer than any
/// settling animation lasts.
const SETTLING_MEMORY: Duration = Duration::from_secs(2);

/// The most inserted ranges [`Dictation::settling`] remembers at once.
const MAX_SETTLING: usize = 8;

/// Characters a segment never takes a space before: closing punctuation.
const CLOSERS: &[char] = &[
	'.', ',', ';', ':', '!', '?', ')', ']', '}', '…', '%', '”', '’', '»',
];

/// Characters a segment never takes a space after: opening brackets and
/// quotes.
const OPENERS: &[char] = &['(', '[', '{', '“', '‘', '«', '"', '\'', '/', '-'];

/// What a speech model said about one segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transcript<'a> {
	/// Text that may still change.
	Partial(&'a str),
	/// Text that never changes again.
	Final(&'a str),
}

/// Text dictation inserted a moment ago, for hosts to fade into ink.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Settling {
	/// The text block holding the text.
	pub block: BlockId,
	/// Where the text starts.
	pub start: u32,
	/// Where the text ends.
	pub end: u32,
	/// When it was inserted, on the clock of [`Env::now`](crate::Env::now).
	pub since: Duration,
}

impl Settling {
	/// Map through `commit`, dropping the range if its text was deleted or
	/// split across blocks.
	fn map(self, commit: &Commit) -> Option<Self> {
		let start = commit.map(Point::new(self.block, self.start), Bias::After);
		let end = commit.map(Point::new(self.block, self.end), Bias::Before);
		let apart = start.point.block != end.point.block || end.point.offset <= start.point.offset;

		if start.deleted || end.deleted || apart {
			return None;
		}

		Some(Self {
			block: start.point.block,
			start: start.point.offset,
			end: end.point.offset,
			since: self.since,
		})
	}
}

/// One segment that has not been inserted yet.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Segment {
	number: u32,
	text: String,
	is_final: bool,
}

/// The dictation state of an editor: segments waiting to be inserted, and
/// text inserted a moment ago.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dictation {
	/// Segments not yet inserted, in segment order.
	pending: Vec<Segment>,
	/// Segments numbered below this were inserted, so news of them is stale.
	floor: u32,
	/// Text inserted a moment ago, oldest first.
	settling: Vec<Settling>,
}

impl Dictation {
	/// Whether any segment is waiting to be inserted.
	#[must_use]
	pub fn is_pending(&self) -> bool {
		!self.pending.is_empty()
	}

	/// Text inserted in the last moments, oldest first, mapped through every
	/// change since.
	#[must_use]
	pub fn settling(&self) -> &[Settling] {
		&self.settling
	}

	/// The provisional text to show at `caret` in `doc`: every waiting
	/// segment, joined to each other and to the text around the caret as it
	/// will be once inserted. `None` when nothing is waiting.
	#[must_use]
	pub fn provisional(&self, doc: &Document, caret: Point) -> Option<String> {
		let (mut before, after) = neighbours(doc, caret);
		let mut text = String::new();

		for segment in &self.pending {
			let Some(first) = segment.text.chars().next() else {
				continue;
			};

			if space_between(before, first) {
				text.push(' ');
			}

			text.push_str(&segment.text);
			before = segment.text.chars().next_back();
		}

		if let (Some(last), Some(after)) = (before, after)
			&& !text.is_empty()
			&& space_between(Some(last), after)
		{
			text.push(' ');
		}

		(!text.is_empty()).then_some(text)
	}

	/// Record `transcript` for segment `number`. Returns whether the
	/// provisional text changed.
	pub(crate) fn record(&mut self, number: u32, transcript: Transcript<'_>) -> bool {
		if number < self.floor {
			return false;
		}

		let (text, is_final) = match transcript {
			Transcript::Partial(text) => (text, false),
			Transcript::Final(text) => (text, true),
		};
		let segment = Segment {
			number,
			text: normalize(text),
			is_final,
		};

		match self
			.pending
			.binary_search_by_key(&number, |segment| segment.number)
		{
			Ok(index) if self.pending[index].is_final => false,
			Ok(index) => {
				let changed = self.pending[index] != segment;
				self.pending[index] = segment;
				changed
			}
			Err(index) => {
				self.pending.insert(index, segment);
				true
			}
		}
	}

	/// The texts of the final segments at the front, in order: the ones
	/// ready to insert.
	fn ready(&self) -> impl Iterator<Item = &str> {
		self.pending
			.iter()
			.take_while(|segment| segment.is_final)
			.map(|segment| segment.text.as_str())
	}

	/// Forget the first `count` segments, which were inserted.
	fn inserted(&mut self, count: usize) {
		let Some(last) = count.checked_sub(1).and_then(|last| self.pending.get(last)) else {
			return;
		};

		self.floor = last.number.saturating_add(1);
		self.pending.drain(..count);
	}

	/// Remember `ranges`, inserted at `now`, and forget ranges that have
	/// long finished settling.
	fn settle(&mut self, ranges: impl IntoIterator<Item = (Point, Point)>, now: Duration) {
		self.settling
			.retain(|range| now.saturating_sub(range.since) < SETTLING_MEMORY);
		self.settling.extend(ranges.into_iter().map(|(start, end)| {
			Settling {
				block: start.block,
				start: start.offset,
				end: end.offset,
				since: now,
			}
		}));

		let excess = self.settling.len().saturating_sub(MAX_SETTLING);
		self.settling.drain(..excess);
	}

	/// Forget every waiting segment, as when dictation stops. Returns whether
	/// provisional text was showing.
	pub(crate) fn stop(&mut self) -> bool {
		let showing = self.is_pending();
		self.pending.clear();
		self.floor = 0;
		showing
	}

	/// Map inserted ranges through `commit`.
	pub(crate) fn map(&mut self, commit: &Commit) {
		if commit.is_empty() || self.settling.is_empty() {
			return;
		}

		self.settling = self
			.settling
			.iter()
			.filter_map(|range| range.map(commit))
			.collect();
	}
}

/// Insert the final segments that are ready, after replacing a selection
/// once anything has been said. All of it is one change, recorded as
/// [`EditKind::Dictation`]. `now` dates the inserted ranges.
pub(crate) fn insert_ready(state: &mut EditorState, now: Duration) -> CommandResult {
	let ready: Vec<String> = state.dictation().ready().map(str::to_owned).collect();
	let replaces = state.dictation().is_pending() && !is_caret(state.selection());

	if ready.is_empty() && !replaces {
		return Ok(None);
	}

	let marks = commands::active_marks(state);
	let mut inserted = Vec::with_capacity(ready.len());

	let update = state.change(EditKind::Dictation, |tr| {
		let Some(mut caret) = replace_selection(tr)? else {
			return Ok(false);
		};

		for text in ready.iter().filter(|text| !text.is_empty()) {
			let (start, end) = insert_segment(tr, caret, text, &marks)?;
			inserted.push((start, end));
			caret = end;
		}

		pad_after(tr, caret, &marks)?;
		tr.set_selection(Selection::caret(caret));
		Ok(true)
	})?;

	if update.is_some() {
		let count = ready.len();
		let dictation = state.dictation_mut();
		dictation.inserted(count);
		dictation.settle(inserted, now);
	}

	Ok(update)
}

/// Whether `selection` is a collapsed text caret.
fn is_caret(selection: Selection) -> bool {
	selection.as_caret().is_some()
}

/// Delete the selected text, returning the caret where it was. A block
/// selection has no caret to dictate at, so dictation waits.
fn replace_selection(tr: &mut EditorTransaction<'_>) -> Result<Option<Point>, crate::EditorError> {
	let Some(range) = position::text_range(tr.doc(), tr.selection_before()) else {
		return Ok(None);
	};

	if range.is_empty() {
		return Ok(Some(range.from));
	}

	structure::delete_range(tr, range.from, range.to).map(Some)
}

/// Insert `text` at `caret` with a space before it when the text before
/// needs one. Returns where the segment's own text starts and ends.
fn insert_segment(
	tr: &mut EditorTransaction<'_>,
	caret: Point,
	text: &str,
	marks: &canvist_model::MarkSet,
) -> Result<(Point, Point), crate::EditorError> {
	let (before, _) = neighbours(tr.doc(), caret);
	let spaced = text
		.chars()
		.next()
		.is_some_and(|first| space_between(before, first));
	let content = if spaced {
		format!(" {text}")
	} else {
		text.to_owned()
	};
	let end = insert_at(tr, caret, InlineText::plain(&content, marks.clone()))?;
	let start = Point::new(end.block, end.offset - text.len() as u32);
	Ok((start, end))
}

/// Insert a space after `caret` when it would otherwise run into the next
/// word, leaving the caret before it so the next segment follows the last.
fn pad_after(
	tr: &mut EditorTransaction<'_>,
	caret: Point,
	marks: &canvist_model::MarkSet,
) -> Result<(), crate::EditorError> {
	let (before, after) = neighbours(tr.doc(), caret);

	if let Some(after) = after
		&& before.is_some()
		&& space_between(before, after)
		&& after.is_alphanumeric()
	{
		insert_at(tr, caret, InlineText::plain(" ", marks.clone()))?;
	}

	Ok(())
}

/// The characters just before and after `caret`.
fn neighbours(doc: &Document, caret: Point) -> (Option<char>, Option<char>) {
	let Some(text) = doc.inline(caret.block).map(InlineText::as_str) else {
		return (None, None);
	};
	let offset = (caret.offset as usize).min(text.len());
	let (before, after) = text.split_at_checked(offset).unwrap_or((text, ""));

	(before.chars().next_back(), after.chars().next())
}

/// Whether one space belongs between `before` (`None` at a block's start)
/// and `next`.
fn space_between(before: Option<char>, next: char) -> bool {
	let Some(before) = before else {
		return false;
	};

	!(before.is_whitespace()
		|| next.is_whitespace()
		|| OPENERS.contains(&before)
		|| CLOSERS.contains(&next))
}

/// `text` with runs of white space, line breaks included, as single spaces,
/// and none at either end.
fn normalize(text: &str) -> String {
	let mut normal = String::with_capacity(text.len());

	for word in text.split_whitespace() {
		if !normal.is_empty() {
			normal.push(' ');
		}
		normal.push_str(word);
	}

	normal
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn spaces_follow_words_but_not_openers_or_line_starts() {
		assert!(space_between(Some('d'), 'n'));
		assert!(!space_between(None, 'n'));
		assert!(!space_between(Some(' '), 'n'));
		assert!(!space_between(Some('('), 'n'));
		assert!(!space_between(Some('d'), '.'));
		assert!(!space_between(Some('d'), ','));
	}

	#[test]
	fn normalizing_collapses_white_space_and_line_breaks() {
		assert_eq!(normalize("  the\n\nmeeting \t is  "), "the meeting is");
		assert_eq!(normalize(" \n "), "");
	}

	#[test]
	fn a_final_segment_never_changes_again() {
		let mut dictation = Dictation::default();
		assert!(dictation.record(0, Transcript::Final("Done.")));
		assert!(!dictation.record(0, Transcript::Partial("done and dusted")));
		assert_eq!(dictation.ready().collect::<Vec<_>>(), ["Done."]);
	}

	#[test]
	fn segments_wait_for_every_earlier_one() {
		let mut dictation = Dictation::default();
		dictation.record(1, Transcript::Final("Second."));
		dictation.record(0, Transcript::Partial("first"));
		assert_eq!(dictation.ready().count(), 0);

		dictation.record(0, Transcript::Final("First."));
		assert_eq!(dictation.ready().collect::<Vec<_>>(), ["First.", "Second."]);
	}

	#[test]
	fn news_of_inserted_segments_is_ignored() {
		let mut dictation = Dictation::default();
		dictation.record(0, Transcript::Final("One."));
		dictation.inserted(1);
		assert!(!dictation.record(0, Transcript::Partial("one again")));
		assert!(!dictation.is_pending());
	}

	#[test]
	fn settling_ranges_are_capped_and_expire() {
		let mut dictation = Dictation::default();
		let block = BlockId::new(1, 1);
		let range = |offset| (Point::new(block, offset), Point::new(block, offset + 1));

		dictation.settle((0..12).map(range), Duration::ZERO);
		assert_eq!(dictation.settling().len(), MAX_SETTLING);
		assert_eq!(dictation.settling()[0].start, 4);

		dictation.settle([range(20)], SETTLING_MEMORY);
		assert_eq!(dictation.settling().len(), 1);
	}
}
