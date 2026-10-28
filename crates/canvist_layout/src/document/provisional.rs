//! Provisional text: text shown in the document that is not part of it yet,
//! such as dictation the speech model may still revise.
//!
//! The block holding it is laid out with the text inserted, so the lines
//! after it reflow exactly as they will once the text is final. Only the
//! laid-out text changes: the document, its offsets, and every query that
//! takes or returns a point stay in document terms.

use std::ops::Range;
use std::sync::Arc;

use canvist_model::InlineText;
use canvist_model::Point;

use super::DocumentLayout;

/// Text laid out at a point in the document without being part of it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Provisional {
	/// Where the text shows. A caret here shows after it.
	pub at: Point,
	/// The text, which takes the marks of the text before it.
	pub text: Arc<str>,
}

impl Provisional {
	/// `text` shown at `at`.
	#[must_use]
	pub fn new(at: Point, text: &str) -> Self {
		Self {
			at,
			text: Arc::from(text),
		}
	}

	/// `inline` with the text inserted, and the bytes it takes, or `None`
	/// when `at` is not an insertion point in `inline`, as when the document
	/// changed and the provisional text has not followed yet.
	pub(crate) fn insert_into(&self, inline: &InlineText) -> Option<(InlineText, Range<u32>)> {
		let offset = self.at.offset;
		let marks = inline
			.marks_before(offset)
			.or_else(|| inline.marks_after(offset))
			.cloned()
			.unwrap_or_default();
		let inserted = InlineText::plain(&self.text, marks);
		let range = offset..offset + inserted.len();
		let mut combined = inline.clone();
		combined.insert(offset, inserted).ok()?;

		Some((combined, range))
	}
}

impl DocumentLayout {
	/// The provisional text, if any.
	#[must_use]
	pub fn provisional(&self) -> Option<&Provisional> {
		self.provisional.as_ref()
	}

	/// Show `provisional` in the document, replacing any provisional text
	/// already showing, or remove it with `None`. Returns whether anything
	/// changed. The blocks it leaves and enters are laid out again by the
	/// next [`DocumentLayout::ensure`].
	///
	/// Set it again after applying commits: a commit that changes the
	/// block's text can leave `at` pointing somewhere else.
	pub fn set_provisional(&mut self, provisional: Option<Provisional>) -> bool {
		if provisional == self.provisional {
			return false;
		}

		let blocks = [&self.provisional, &provisional]
			.map(|shown| shown.as_ref().map(|shown| shown.at.block));

		for block in blocks.into_iter().flatten() {
			let Some(index) = self.row_index(block) else {
				continue;
			};
			let row = &mut self.flat.rows[index];
			row.layout = None;
			row.previous = None;
		}

		self.provisional = provisional;
		true
	}
}

#[cfg(test)]
mod tests {
	use canvist_model::BlockId;
	use canvist_model::MarkSet;

	use super::*;

	#[test]
	fn inserting_reports_the_bytes_the_text_takes() {
		let inline = InlineText::plain("Hello", MarkSet::default());
		let provisional = Provisional::new(Point::new(BlockId::new(1, 1), 5), " world");
		let (combined, range) = provisional.insert_into(&inline).expect("inserts");

		assert_eq!(combined.as_str(), "Hello world");
		assert_eq!(range, 5..11);
	}

	#[test]
	fn a_point_past_the_end_inserts_nothing() {
		let inline = InlineText::plain("Hi", MarkSet::default());
		let provisional = Provisional::new(Point::new(BlockId::new(1, 1), 9), "there");

		assert_eq!(provisional.insert_into(&inline), None);
	}
}
