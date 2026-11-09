#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_clipboard`
//!
//! The canvist editor's clipboard formats: HTML, Markdown, and plain text.
//! [`Editor::copy`](canvist_core::Editor::copy) produces a [`Slice`]; these
//! functions turn it into the formats a system clipboard carries, and turn
//! pasted content back into a slice for
//! [`Editor::paste`](canvist_core::Editor::paste).
//!
//! - [`to_html`] writes the markup TipTap renders, and [`from_html`] reads
//!   it back, along with what browsers, Google Docs, and Word put on the
//!   clipboard.
//! - [`to_markdown`] writes CommonMark with GitHub's strikethrough, task
//!   lists, pipe tables, and alerts, and [`from_markdown`] reads it.
//! - [`to_plain_text`] writes one line per text block, and
//!   [`from_plain_text`] reads one paragraph per line.
//!
//! Every reader takes the [`Schema`] the slice is for, and builds only the
//! types it has: content without a type falls back to paragraphs and plain
//! text, and a bare paragraph in a list gets a list item. Writers know the
//! TipTap names (`heading`, `bulletList`, `bold`, ...) and write unknown
//! blocks as their content.
//!
//! The infallible readers preserve input that nests deeper than
//! [`MAX_PARSE_DEPTH`] as literal plain text; [`try_from_html`] and
//! [`try_from_markdown`] return [`NestingLimitExceeded`] instead.
//!
//! The parsers are small and hand-written rather than full HTML5 or
//! CommonMark implementations: clipboard content only needs its structure
//! and formatting read, and a full HTML parser would add hundreds of
//! kilobytes to every WebAssembly build.
//!
//! # Features
//!
//! The StarterKit's markup is always understood. Each feature adds the
//! markup of the extension of the same name, which is otherwise read and
//! written as plain HTML or Markdown; `canvist_extensions` turns on the
//! feature with the extension.
//!
//! - `audio`, `file`, `image`, `twitch`, and `youtube`: media blocks as
//!   `<audio>`, file cards, `<img>`, and video frames, and in Markdown as
//!   images and links. A video's frame only keeps a source on its own site.
//! - `callout`: `<div data-callout-type>`, and GitHub alerts such as
//!   `> [!WARNING]` in Markdown.
//! - `comments`, `conditional`, `hashtag`, and `kinetic`: their marks as
//!   `<span>`s with TipTap's `data-` attributes.
//! - `details`: `<details>` and `<summary>`, in Markdown too.
//! - `emoji` and `mention`: inline `<span data-type>` atoms, written as
//!   the text they show elsewhere.
//! - `highlight`: `<mark>`, and `==text==` in Markdown.
//! - `ink`: drawings as a `<figure>` holding their strokes and an SVG.
//! - `mathematics`: formulas as `<span>` and `<div data-type="…-math">`,
//!   and `$…$` and `$$` blocks in Markdown.
//! - `subscript` and `superscript`: `<sub>` and `<sup>`, in Markdown too.
//! - `table`: `<table>` with spans and column widths, and GitHub pipe
//!   tables in Markdown when a table fits one.
//! - `text-align`: a block's `text-align` style.
//! - `text-style`: `<span style>` colours, fonts, and sizes, which only
//!   ever write values from a small, safe CSS grammar.
//!
//! # Example
//!
//! ```
//! use canvist_core::Editor;
//! use canvist_extension_bold::Bold;
//! use canvist_extension_clipboard::from_markdown;
//! use canvist_extension_clipboard::to_html;
//! use canvist_extension_clipboard::to_plain_text;
//! use canvist_extension_document::Document;
//! use canvist_extension_link::Link;
//! use canvist_extension_paragraph::Paragraph;
//!
//! let schema = Editor::builder()
//! 	.extension(Document)
//! 	.extension(Paragraph)
//! 	.extension(Bold)
//! 	.extension(Link::default())
//! 	.schema()?;
//! let slice = from_markdown(&schema, "Some **bold** and a [link](https://canvist.dev)");
//!
//! assert_eq!(
//! 	to_html(&schema, &slice),
//! 	"<p>Some <strong>bold</strong> and a <a href=\"https://canvist.dev\">link</a></p>"
//! );
//! assert_eq!(to_plain_text(&schema, &slice), "Some bold and a link");
//! # Ok::<(), canvist_core::EditorError>(())
//! ```

mod css;
mod features;
mod tree;

mod html_read;
mod html_role;
mod html_token;
mod html_write;

mod markdown_inline;
mod markdown_read;
mod markdown_write;

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
