//! Clipboard formats: HTML, Markdown, and plain text.
//!
//! [`Editor::copy`](canvist_core::Editor::copy) produces a [`Slice`]; these
//! functions turn it into the formats a system clipboard carries, and turn
//! pasted content back into a slice for
//! [`Editor::paste`](canvist_core::Editor::paste).
//!
//! The codecs know the StarterKit's TipTap names (`heading`, `bulletList`,
//! `bold`, ...). Unknown block types serialize as their text, and parsed
//! content that the schema lacks a type for falls back to paragraphs and
//! plain text. The infallible readers preserve input that exceeds
//! [`MAX_PARSE_DEPTH`] as literal plain text; the `try_from_*` readers
//! return [`NestingLimitExceeded`] instead.
//!
//! The parsers are small and hand-written rather than full HTML5 or
//! CommonMark implementations: clipboard content only needs its structure
//! and formatting read, and a full HTML parser would add hundreds of
//! kilobytes to every WebAssembly build.

mod html_read;
mod html_write;
mod markdown_inline;
mod markdown_read;
mod markdown_write;
mod tree;

use canvist_core::Slice;
use canvist_model::BlockTree;
use canvist_model::Schema;
use canvist_model::TreeContent;
pub use html_read::from_html;
pub use html_read::try_from_html;
pub use html_write::to_html;
pub use markdown_read::from_markdown;
pub use markdown_read::try_from_markdown;
pub use markdown_write::to_markdown;

/// Maximum active parser levels, shared by nested Markdown blocks,
/// inline syntax, and embedded HTML. The root reader counts as one level.
///
/// This also bounds trees before recursive schema fitting and disposal.
pub const MAX_PARSE_DEPTH: usize = 32;

/// Input exceeded [`MAX_PARSE_DEPTH`]. No partial slice is returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NestingLimitExceeded;

impl std::fmt::Display for NestingLimitExceeded {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			formatter,
			"nesting exceeds the parser limit of {MAX_PARSE_DEPTH} levels"
		)
	}
}

impl std::error::Error for NestingLimitExceeded {}

/// The slice's text: one line per text block, with hard breaks as line
/// breaks and other inline embeds dropped.
#[must_use]
pub fn to_plain_text(schema: &Schema, slice: &Slice) -> String {
	let mut lines = Vec::new();

	for tree in slice.blocks() {
		tree.walk(&mut |node: &BlockTree| {
			if let TreeContent::Inline(text) = &node.content {
				lines.push(tree::plain_text(schema, text));
			}
		});
	}

	lines.join("\n")
}

/// One paragraph per line of `text`.
#[must_use]
pub fn from_plain_text(schema: &Schema, text: &str) -> Slice {
	Slice::from_plain_text(schema, text)
}
