//! Adornments: text an extension shows in the document without it being
//! part of the document, such as the Markdown syntax around formatting at
//! the caret, or an inline suggestion.
//!
//! An adornment sits at a point, between the two carets layout can draw
//! there: a caret at the point with [`Affinity::Upstream`](crate::Affinity)
//! shows before it, one with [`Affinity::Downstream`](crate::Affinity) after
//! it. Nothing about the document changes, so undo, collaboration, and
//! export never see adornments.

use std::sync::Arc;

use canvist_model::Point;

/// Text shown at a point without being part of the document.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Adornment {
	/// Where the text shows.
	pub at: Point,
	/// The text, drawn plain and faint in the block's style.
	pub text: Arc<str>,
}

impl Adornment {
	/// `text` shown at `at`.
	#[must_use]
	pub fn new(at: Point, text: &str) -> Self {
		Self {
			at,
			text: Arc::from(text),
		}
	}
}
