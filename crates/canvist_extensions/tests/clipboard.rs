//! Clipboard formats: serializing copied content and pasting it back.

#![cfg(feature = "clipboard")]

mod support;

use std::time::Duration;

use canvist_core::Env;
use canvist_core::Slice;
use canvist_extensions::clipboard;
use insta::assert_json_snapshot;
use insta::assert_snapshot;
use support::Typist;

/// A document using every StarterKit feature, fully selected.
const RICH: &str = "# Title⏎Some **bold**, *italic*, ~~gone~~, `code` and [Mod-u]under[Mod-u] text[Shift-Enter]next line⏎- one⏎two⇥⏎⏎⏎1. first⏎⏎> quoted⏎⏎```rust let x = 1;⏎⏎⏎[[x] done⏎⏎---see https://canvist.dev now[Mod-a]";

fn copied() -> (Typist, Slice) {
	let mut typist = Typist::new();
	typist.type_script(RICH);
	let slice = typist.editor.copy().expect("copy");
	(typist, slice)
}

/// Paste `slice` into a fresh editor and snapshot it.
fn pasted(slice: Slice) -> serde_json::Value {
	let mut typist = Typist::new();
	typist
		.editor
		.paste(slice, &mut Env::headless(Duration::ZERO))
		.expect("paste");
	typist.snapshot()["doc"].clone()
}

#[test]
fn rich_content_to_html() {
	let (typist, slice) = copied();

	assert_snapshot!(clipboard::to_html(typist.editor.schema(), &slice));
}

#[test]
fn rich_content_to_markdown() {
	let (typist, slice) = copied();

	assert_snapshot!(clipboard::to_markdown(typist.editor.schema(), &slice));
}

#[test]
fn rich_content_to_plain_text() {
	let (typist, slice) = copied();

	assert_snapshot!(clipboard::to_plain_text(typist.editor.schema(), &slice));
}

#[test]
fn html_round_trips_through_paste() {
	let (typist, slice) = copied();
	let schema = typist.editor.schema();
	let html = clipboard::to_html(schema, &slice);
	let parsed = clipboard::from_html(schema, &html);

	assert_eq!(clipboard::to_html(schema, &parsed), html);
	assert_eq!(pasted(parsed), typist.snapshot()["doc"]);
}

#[test]
fn imported_links_with_executable_schemes_export_without_href() {
	let typist = Typist::new();
	let schema = typist.editor.schema();

	for href in [
		"javascript:alert(1)",
		" JaVaScRiPt:alert(1)",
		"java\tscript:alert(1)",
		"java\nscript:alert(1)",
		"java\rscript:alert(1)",
		"java&#x73;cript:alert(1)",
		"java&#9;script:alert(1)",
		"data:text/html,script",
		"vbscript:msgbox(1)",
	] {
		let html = format!("<p><a href=\"{href}\"><strong>link</strong></a></p>");
		let slice = clipboard::from_html(schema, &html);

		assert_eq!(
			clipboard::to_html(schema, &slice),
			"<p><strong><a>link</a></strong></p>",
			"{href:?}"
		);
	}
}

#[test]
fn markdown_round_trips_except_underline() {
	let (typist, slice) = copied();
	let schema = typist.editor.schema();
	let markdown = clipboard::to_markdown(schema, &slice);
	let parsed = clipboard::from_markdown(schema, &markdown);

	assert_eq!(clipboard::to_markdown(schema, &parsed), markdown);
}

#[test]
fn pastes_word_processor_html() {
	let html = r#"<meta charset="utf-8"><style>p { margin: 0 }</style>
		<b style="font-weight:normal;" id="docs-internal-guid">
		<h2 dir="ltr"><span>Plan&nbsp;</span></h2>
		<p dir="ltr"><span>Ship </span><span><strong>fast</strong></span><span> &amp; <em>well</em></span></p>
		<ul><li><p>one</p></li><li>two<ul><li>nested</li></ul></li></ul>
		<ol start="4"><li>four</li></ol>
		<table><tr><td>cell</td></tr></table>
		<p>line<br>break</p><img src="x.png"><hr>
		</b>"#;
	let typist = Typist::new();
	let slice = clipboard::from_html(typist.editor.schema(), html);

	assert_json_snapshot!(pasted(slice));
}

#[test]
fn pastes_tiptap_task_lists_and_code() {
	let html = r#"<ul data-type="taskList"><li data-type="taskItem" data-checked="true"><p>done</p></li><li data-type="taskItem" data-checked="false"><p>todo</p></li></ul><pre><code class="language-rust">fn main() {
    println!("&lt;hi&gt;");
}</code></pre><blockquote><p>q</p></blockquote>"#;
	let typist = Typist::new();
	let slice = clipboard::from_html(typist.editor.schema(), html);

	assert_json_snapshot!(pasted(slice));
}

#[test]
fn pastes_markdown() {
	let markdown = "## Notes\n\nSome *emphasis*, **strong**, ~~struck~~, `code`, and a [link](https://canvist.dev).\nSoft break and hard\\\nbreak.\n\n- a\n- [b](x)\n  - nested\n\n3. three\n4. four\n\n- [ ] task\n- [x] done\n\n> quoted\n> lines\n\n```js\nconst x = 1;\n```\n\n***\n\nsnake_case_word and 2*3*4\n";
	let typist = Typist::new();
	let slice = clipboard::from_markdown(typist.editor.schema(), markdown);

	assert_json_snapshot!(pasted(slice));
}

#[test]
fn unknown_markup_degrades_to_text() {
	let typist = Typist::new();
	let schema = typist.editor.schema();
	let slice = clipboard::from_html(
		schema,
		"<custom-el>a<x-y>b</x-y></custom-el><p>c &bogus; d</p>stray",
	);

	assert_eq!(
		clipboard::to_plain_text(schema, &slice),
		"ab\nc &bogus; d\nstray"
	);
	assert!(clipboard::from_html(schema, "").is_empty());
	assert!(clipboard::from_markdown(schema, "\n\n").is_empty());
}

#[test]
fn markdown_escapes_text_that_looks_like_syntax() {
	let typist = Typist::new();
	let schema = typist.editor.schema();
	let slice = clipboard::from_plain_text(
		schema,
		"# not a heading\n1. not a list\n*stars* and [brackets]",
	);
	let markdown = clipboard::to_markdown(schema, &slice);
	let parsed = clipboard::from_markdown(schema, &markdown);

	assert_eq!(
		clipboard::to_plain_text(schema, &parsed),
		"# not a heading\n1. not a list\n*stars* and [brackets]"
	);
}

#[test]
fn reads_inline_styles_from_word_processors() {
	let html = r#"<b style="font-weight:normal"><span style="font-weight:700">bold</span> <span style="font-style:italic;text-decoration:underline line-through">mixed</span></b>"#;
	let typist = Typist::new();
	let schema = typist.editor.schema();
	let slice = clipboard::from_html(schema, html);

	assert_eq!(
		clipboard::to_html(schema, &slice),
		"<p><strong>bold</strong> <em><u><s>mixed</s></u></em></p>"
	);
}

#[test]
fn excessive_nesting_is_preserved_as_plain_text() {
	let typist = Typist::new();
	let schema = typist.editor.schema();
	let markdown = format!("{}text", "> ".repeat(64));
	let html = format!(
		"{}text{}",
		"<blockquote>".repeat(64),
		"</blockquote>".repeat(64)
	);

	assert_eq!(
		clipboard::to_plain_text(schema, &clipboard::from_markdown(schema, &markdown)),
		markdown
	);
	assert_eq!(
		clipboard::to_plain_text(schema, &clipboard::from_html(schema, &html)),
		html
	);
}

/// Nested link labels exercise recursion even when a mark is already active.
fn nested_links(depth: usize) -> String {
	format!("{}text{}", "[".repeat(depth), "](x)".repeat(depth))
}

#[test]
fn markdown_depth_boundaries_cover_blocks_and_inline_syntax() {
	let typist = Typist::new();
	let schema = typist.editor.schema();
	let limit = clipboard::MAX_PARSE_DEPTH;

	for depth in [limit - 3, limit - 2, limit - 1] {
		for markdown in [
			format!("{}text", "> ".repeat(depth)),
			format!("{}text", "- ".repeat(depth)),
			nested_links(depth),
		] {
			let result = clipboard::try_from_markdown(schema, &markdown);

			if depth + 2 <= limit {
				let slice = result.expect("nesting within budget");
				assert_eq!(clipboard::to_plain_text(schema, &slice), "text");
			} else {
				assert_eq!(result.err(), Some(clipboard::NestingLimitExceeded));
			}
		}
	}
}

#[test]
fn html_depth_boundaries_cover_containers_and_marks() {
	let typist = Typist::new();
	let schema = typist.editor.schema();
	let limit = clipboard::MAX_PARSE_DEPTH;

	for depth in [limit - 2, limit - 1, limit] {
		for tag in ["blockquote", "strong"] {
			let html = format!(
				"{}text{}",
				format!("<{tag}>").repeat(depth),
				format!("</{tag}>").repeat(depth)
			);
			let result = clipboard::try_from_html(schema, &html);

			if depth < limit {
				let slice = result.expect("nesting within budget");
				assert_eq!(clipboard::to_plain_text(schema, &slice), "text");
			} else {
				assert_eq!(result.err(), Some(clipboard::NestingLimitExceeded));
			}
		}
	}
}

#[test]
fn deeply_nested_input_returns_errors_on_a_small_stack() {
	std::thread::Builder::new()
		.stack_size(1024 * 1024)
		.spawn(|| {
			let typist = Typist::new();
			let schema = typist.editor.schema();
			let accepted = clipboard::MAX_PARSE_DEPTH - 2;

			for marker in ["> ", "- "] {
				let markdown = format!("{}text", marker.repeat(accepted));
				assert!(clipboard::try_from_markdown(schema, &markdown).is_ok());
			}

			for markdown in [
				format!("{}text", "> ".repeat(10_000)),
				format!("{}text", "- ".repeat(10_000)),
				nested_links(10_000),
			] {
				assert_eq!(
					clipboard::try_from_markdown(schema, &markdown).err(),
					Some(clipboard::NestingLimitExceeded)
				);
			}

			let html = format!(
				"{}text{}",
				"<blockquote>".repeat(10_000),
				"</blockquote>".repeat(10_000)
			);
			assert_eq!(
				clipboard::try_from_html(schema, &html).err(),
				Some(clipboard::NestingLimitExceeded)
			);
		})
		.expect("start parser thread")
		.join()
		.expect("parsers return without overflowing");
}

#[test]
fn mixed_markdown_nesting_shares_one_budget() {
	let typist = Typist::new();
	let schema = typist.editor.schema();
	let markdown = format!("{}{}", "> ".repeat(16), nested_links(16));

	assert_eq!(
		clipboard::try_from_markdown(schema, &markdown).err(),
		Some(clipboard::NestingLimitExceeded)
	);
}

#[cfg(feature = "table")]
#[test]
fn markdown_embedded_html_shares_the_enclosing_budget() {
	let typist = Typist::new();
	let schema = typist.editor.schema();
	let html = format!(
		"<table>{}text{}</table>",
		"<blockquote>".repeat(16),
		"</blockquote>".repeat(16)
	);
	let markdown = format!("{}{html}", "> ".repeat(16));

	assert!(clipboard::try_from_html(schema, &html).is_ok());
	assert_eq!(
		clipboard::try_from_markdown(schema, &markdown).err(),
		Some(clipboard::NestingLimitExceeded)
	);
}

#[test]
fn sibling_tags_release_their_budget() {
	let typist = Typist::new();
	let schema = typist.editor.schema();
	let html = "<blockquote><p><strong>text</strong></p></blockquote>".repeat(100);
	let markdown = "> [text](x)\n\n".repeat(100);

	assert!(clipboard::try_from_html(schema, &html).is_ok());
	assert!(clipboard::try_from_markdown(schema, &markdown).is_ok());
}

#[cfg(feature = "table")]
#[test]
fn unclosed_html_marks_release_the_embedded_reader_budget() {
	let typist = Typist::new();
	let markdown = "<table><strong>text</table>\n\n".repeat(100);

	assert!(clipboard::try_from_markdown(typist.editor.schema(), &markdown).is_ok());
}

#[cfg(feature = "details")]
#[test]
fn nested_markdown_details_share_the_budget() {
	let typist = Typist::new();
	let markdown = format!(
		"{}text\n{}",
		"<details>\n".repeat(10_000),
		"</details>\n".repeat(10_000)
	);

	assert_eq!(
		clipboard::try_from_markdown(typist.editor.schema(), &markdown).err(),
		Some(clipboard::NestingLimitExceeded)
	);
}

#[test]
fn missing_schema_types_do_not_bypass_the_budget() {
	let editor = canvist_core::Editor::builder()
		.extension(canvist_extensions::Document)
		.extension(canvist_extensions::Paragraph)
		.build()
		.expect("plain schema");
	let schema = editor.schema();
	let markdown = nested_links(10_000);
	let html = "<blockquote>".repeat(10_000);

	assert_eq!(
		clipboard::try_from_markdown(schema, &markdown).err(),
		Some(clipboard::NestingLimitExceeded)
	);
	assert_eq!(
		clipboard::try_from_html(schema, &html).err(),
		Some(clipboard::NestingLimitExceeded)
	);
}

/// Plain text through Markdown: written, read back, and its text again.
fn markdown_text(text: &str) -> (String, String) {
	let typist = Typist::new();
	let schema = typist.editor.schema();
	let markdown = clipboard::to_markdown(schema, &clipboard::from_plain_text(schema, text));
	let read = clipboard::to_plain_text(schema, &clipboard::from_markdown(schema, &markdown));
	(markdown, read)
}

#[test]
fn markdown_escapes_only_what_would_read_as_syntax() {
	for (text, markdown) in [
		("snake_case_word and a * b", "snake_case_word and a * b"),
		("Not fit to print [a note]", "Not fit to print [a note]"),
		(
			"see https://canvist.dev/_a_b_?q=1~2 now",
			"see https://canvist.dev/_a_b_?q=1~2 now",
		),
		("C:\\Users\\me and 50% off!", "C:\\Users\\me and 50% off!"),
		("a lone ` backtick", "a lone ` backtick"),
		("*stars* and _under_", "\\*stars\\* and \\_under\\_"),
		("[label](href) is text", "\\[label\\](href) is text"),
		("`one` and `two`", "\\`one\\` and \\`two\\`"),
		("a trailing backslash \\", "a trailing backslash \\\\"),
		("~~struck~~", "\\~\\~struck\\~\\~"),
	] {
		let (written, read) = markdown_text(text);
		assert_eq!(written, markdown, "{text}");
		assert_eq!(read, text, "{text} reads back");
	}
}

#[test]
fn markdown_bare_urls_are_text() {
	let typist = Typist::new();
	let schema = typist.editor.schema();

	for markdown in [
		"see https://canvist.dev/_a_ now",
		"see https://canvist.dev/\\_a\\_ now",
	] {
		let slice = clipboard::from_markdown(schema, markdown);
		assert_eq!(
			clipboard::to_plain_text(schema, &slice),
			"see https://canvist.dev/_a_ now",
			"{markdown}"
		);
		assert!(
			!clipboard::to_html(schema, &slice).contains("<em>"),
			"{markdown} isn't italic"
		);
	}

	// Outside a URL the same text is emphasis.
	let html = clipboard::to_html(schema, &clipboard::from_markdown(schema, "see /_a_ now"));
	assert!(html.contains("<em>a</em>"), "{html}");
}

#[test]
fn markdown_two_trailing_spaces_make_a_hard_break() {
	let typist = Typist::new();
	let schema = typist.editor.schema();
	let read = |markdown: &str| {
		clipboard::to_markdown(schema, &clipboard::from_markdown(schema, markdown))
	};

	assert_eq!(read("one  \ntwo"), "one\\\ntwo");
	assert_eq!(read("one \ntwo"), "one two");
	assert_eq!(read("one\ntwo  "), "one two");
}

proptest::proptest! {
	/// Any text, however much of it looks like Markdown, reads back as the
	/// same text after a trip through Markdown.
	#[test]
	fn markdown_keeps_punctuation_heavy_text(text in "[a-z \\\\*_`~\\[\\]()!#:<>=$|.-]{1,24}") {
		let (written, read) = markdown_text(&text);
		proptest::prop_assert_eq!(read, text.trim().to_owned(), "{}", written);
	}
}
