//! Malformed and hostile clipboard content, which must read and write
//! without panics, runaway allocations, or markup leaking out of where it
//! belongs.

use std::sync::Arc;

use canvist_core::Editor;
use canvist_extension_bold::Bold;
use canvist_extension_clipboard::from_html;
use canvist_extension_clipboard::from_markdown;
use canvist_extension_clipboard::to_html;
use canvist_extension_clipboard::to_markdown;
use canvist_extension_code_block::CodeBlock;
use canvist_extension_document::Document;
use canvist_extension_link::Link;
use canvist_extension_list::ListItem;
use canvist_extension_list::OrderedList;
use canvist_extension_paragraph::Paragraph;
use canvist_model::Schema;

fn schema() -> Arc<Schema> {
	Editor::builder()
		.extension(Document)
		.extension(Paragraph)
		.extension(Bold)
		.extension(Link::default())
		.extension(CodeBlock)
		.extension(ListItem)
		.extension(OrderedList)
		.extension(canvist_extension_image::Image)
		.extension(canvist_extension_table::Table)
		.schema()
		.expect("the extensions fit together")
}

/// Regression: the tag ended at the first `>`, so a `title` holding one
/// lost the link's `href` and spilled the rest of the tag into the text.
#[test]
fn a_quoted_angle_bracket_does_not_end_a_tag() {
	let schema = schema();
	let slice = from_html(
		&schema,
		"<p><a title=\"1 > 0\" href=\"https://canvist.dev\">link</a></p>",
	);

	assert_eq!(
		to_html(&schema, &slice),
		"<p><a href=\"https://canvist.dev\">link</a></p>"
	);
}

#[test]
fn a_quote_that_never_closes_ends_the_tag_at_the_first_angle_bracket() {
	let schema = schema();
	let slice = from_html(&schema, "<p title=\"x>kept</p>");

	assert_eq!(to_html(&schema, &slice), "<p>kept</p>");
}

/// Regression: the end of a script was searched for in lowercase only, so
/// `</SCRIPT>` was never found and everything after it was dropped.
#[test]
fn scripts_end_whatever_the_case_of_their_closing_tag() {
	let schema = schema();
	let slice = from_html(&schema, "<SCRIPT>alert(1)</Script><p>kept</p>");

	assert_eq!(to_html(&schema, &slice), "<p>kept</p>");
}

/// Regression: a link destination ended at its first `)`, so Wikipedia's
/// URLs lost their last character and left a stray parenthesis.
#[test]
fn link_destinations_hold_parentheses_in_pairs_and_escaped() {
	let schema = schema();
	let slice = from_markdown(
		&schema,
		"[Rust](https://en.wikipedia.org/wiki/Rust_(language)) [x](a\\)b)",
	);

	assert_eq!(
		to_html(&schema, &slice),
		"<p><a href=\"https://en.wikipedia.org/wiki/Rust_(language)\">Rust</a> <a \
		 href=\"a)b\">x</a></p>"
	);
}

/// Regression: an `href` was written into Markdown as it was, so one with
/// a `)` ended the link early and the rest of it became Markdown.
#[test]
fn a_link_destination_cannot_close_early_or_add_markup() {
	let schema = schema();
	let slice = from_html(
		&schema,
		"<p><a href=\"https://a.example/x) [click](https://b.example\">text</a></p>",
	);
	let markdown = to_markdown(&schema, &slice);

	assert_eq!(
		markdown,
		"[text](https://a.example/x\\)%20[click]\\(https://b.example)"
	);
	assert_eq!(
		to_html(&schema, &from_markdown(&schema, &markdown)),
		"<p><a href=\"https://a.example/x)%20[click](https://b.example\">text</a></p>",
		"it reads back as the one link"
	);
}

#[test]
fn links_with_executable_schemes_write_their_text_without_a_target() {
	let schema = schema();
	let slice = from_html(&schema, "<p><a href=\"javascript:alert(1)\">text</a></p>");

	assert_eq!(to_markdown(&schema, &slice), "text");
}

/// Regression: code holding a backtick fence was fenced with `~~~`
/// whatever else it held, so a `~~~` line inside closed it and the rest of
/// the code was read as Markdown.
#[test]
fn code_holding_both_fences_round_trips_through_markdown() {
	let schema = schema();
	let slice = from_html(&schema, "<pre><code>~~~\n```\n# not a heading</code></pre>");
	let markdown = to_markdown(&schema, &slice);

	assert_eq!(markdown, "~~~~\n~~~\n```\n# not a heading\n~~~~");
	assert_eq!(
		to_html(&schema, &from_markdown(&schema, &markdown)),
		to_html(&schema, &slice)
	);
}

/// Regression: a language holding a line break ended the opening fence,
/// and the rest of it became a heading.
#[test]
fn a_code_language_stays_on_the_fence_line() {
	let schema = schema();
	let slice = from_html(
		&schema,
		"<pre><code class=\"language-rust&#10;# Heading\">x</code></pre>",
	);

	assert_eq!(to_markdown(&schema, &slice), "```rust\nx\n```");
}

/// Regression: numbering the items of a list starting at the largest
/// number overflowed, which panics in debug builds.
#[test]
fn an_ordered_list_starting_at_the_largest_number_writes_without_overflow() {
	let schema = schema();
	let slice = from_html(
		&schema,
		"<ol start=\"9223372036854775807\"><li><p>a</p></li><li><p>b</p></li></ol>",
	);

	assert_eq!(
		to_markdown(&schema, &slice),
		"9223372036854775807. a\n9223372036854775807. b"
	);
}

/// Regression: spans were kept however large, and a table grid of four
/// billion columns cannot be allocated.
#[cfg(feature = "table")]
#[test]
fn pasted_spans_are_capped_as_browsers_cap_them() {
	let schema = schema();
	let slice = from_html(
		&schema,
		"<table><tr><td colspan=\"4000000000\" rowspan=\"99999\"><p>a</p></td></tr></table>",
	);

	assert_eq!(
		to_html(&schema, &slice),
		"<table><tbody><tr><td colspan=\"1000\" \
		 rowspan=\"65534\"><p>a</p></td></tr></tbody></table>"
	);
}

/// Regression: image alt text was read with its escapes, so each round
/// trip through Markdown added backslashes.
#[cfg(feature = "image")]
#[test]
fn image_text_and_sources_round_trip_with_escapes() {
	let schema = schema();
	let markdown = "![a\\*b \\[c\\]](x\\(1\\).png \"say \\\"hi\\\"\")";
	let slice = from_markdown(&schema, markdown);

	assert_eq!(
		to_html(&schema, &slice),
		"<img src=\"x(1).png\" alt=\"a*b [c]\" title=\"say &quot;hi&quot;\">"
	);
	assert_eq!(to_markdown(&schema, &slice), markdown);
}

#[cfg(feature = "image")]
#[test]
fn image_alt_text_stays_on_one_line_in_markdown() {
	let schema = schema();
	let slice = from_html(&schema, "<img src=\"x.png\" alt=\"a&#10;&#10;<b>raw</b>\">");

	assert_eq!(to_markdown(&schema, &slice), "![a  <b>raw</b>](x.png)");
}
