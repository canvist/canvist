//! Markdown reveal: the syntax of the formatting at the caret shows around
//! it, the arrow keys stop on both sides of it, typing follows the side the
//! caret is on, and Backspace and Delete edit it.
//!
//! [`shown`] writes a block the way it looks: its text, the revealed syntax
//! between `‹` and `›`, and the caret as `|` on its side of the syntax.

#![cfg(all(feature = "markdown-reveal", feature = "clipboard"))]

mod support;

use std::sync::Arc;
use std::time::Duration;

use canvist_core::Affinity;
use canvist_core::Env;
use canvist_core::Slice;
use canvist_extensions::MarkdownReveal;
use canvist_extensions::clipboard;
use canvist_model::Point;
use canvist_model::Selection;
use support::Typist;
use support::editor_with;

fn typist() -> Typist {
	Typist::with(editor_with(vec![Arc::new(MarkdownReveal)]))
}

/// A typist whose first paragraph is `markdown`, read as Markdown.
fn from_markdown(markdown: &str) -> Typist {
	let mut typist = typist();
	let slice = clipboard::from_markdown(typist.editor.schema(), markdown);
	typist
		.editor
		.paste(slice, &mut Env::headless(Duration::ZERO))
		.expect("paste");
	typist
}

fn first_block(typist: &Typist) -> canvist_model::BlockId {
	typist
		.editor
		.state()
		.doc()
		.first_text_block()
		.expect("a text block")
}

/// Put the caret at byte `offset` of the first block.
fn caret_at(typist: &mut Typist, offset: u32) {
	let point = Point::new(first_block(typist), offset);
	typist
		.editor
		.select(
			Selection::Text {
				anchor: point,
				head: point,
			},
			&mut Env::headless(Duration::ZERO),
		)
		.expect("select");
}

/// The first block as it shows: syntax in `‹›`, the caret as `|`.
fn shown(typist: &Typist) -> String {
	let state = typist.editor.state();
	let block = first_block(typist);
	let text = state.doc().inline(block).expect("text").as_str().to_owned();
	let caret = state
		.selection()
		.as_caret()
		.filter(|point| point.block == block);
	let adornments = typist.editor.adornments();
	let mut out = String::new();

	for offset in 0..=text.len() as u32 {
		let mut syntax = String::new();

		for adornment in adornments
			.iter()
			.filter(|adornment| adornment.at.block == block && adornment.at.offset == offset)
		{
			syntax.push('‹');
			syntax.push_str(&adornment.text);
			syntax.push('›');
		}

		let here = caret.filter(|point| point.offset == offset);

		match here.map(|_| state.affinity()) {
			Some(Affinity::Upstream) => {
				out.push('|');
				out.push_str(&syntax);
			}
			Some(Affinity::Downstream) => {
				out.push_str(&syntax);
				out.push('|');
			}
			None => out.push_str(&syntax),
		}

		if let Some(character) = text[offset as usize..].chars().next() {
			out.push(character);
		}
	}

	out
}

/// The whole document as Markdown.
fn markdown(typist: &Typist) -> String {
	let doc = typist.editor.state().doc();
	let slice = Slice::new(
		doc.children(None)
			.iter()
			.filter_map(|&id| doc.tree(id))
			.collect(),
	);
	clipboard::to_markdown(typist.editor.schema(), &slice)
}

#[test]
fn syntax_shows_only_while_the_caret_touches_the_formatting() {
	let mut typist = from_markdown("Say **bold** now");

	caret_at(&mut typist, 12);
	assert_eq!(shown(&typist), "Say bold now|");
	caret_at(&mut typist, 6);
	assert_eq!(shown(&typist), "Say ‹**›bo|ld‹**› now");
	// Touching either edge counts.
	caret_at(&mut typist, 8);
	assert_eq!(shown(&typist), "Say ‹**›bold‹**›| now");
}

#[test]
fn the_arrow_keys_stop_on_both_sides_of_the_syntax() {
	let mut typist = from_markdown("Say **bold** now");
	caret_at(&mut typist, 3);
	let mut seen = vec![shown(&typist)];

	for _ in 0..7 {
		typist.type_script("→");
		seen.push(shown(&typist));
	}

	assert_eq!(
		seen,
		[
			"Say| bold now",
			"Say |‹**›bold‹**› now",
			"Say ‹**›|bold‹**› now",
			"Say ‹**›b|old‹**› now",
			"Say ‹**›bo|ld‹**› now",
			"Say ‹**›bol|d‹**› now",
			"Say ‹**›bold|‹**› now",
			"Say ‹**›bold‹**›| now",
		]
	);

	typist.type_script("→");
	assert_eq!(shown(&typist), "Say bold |now");

	for expected in [
		"Say ‹**›bold‹**›| now",
		"Say ‹**›bold|‹**› now",
		"Say ‹**›bol|d‹**› now",
	] {
		typist.type_script("←");
		assert_eq!(shown(&typist), expected);
	}
}

#[test]
fn typing_follows_the_side_of_the_syntax_the_caret_is_on() {
	let mut typist = from_markdown("Say **bold** now");

	caret_at(&mut typist, 4);
	typist.type_script("←"); // Before the opening `**`.
	assert_eq!(shown(&typist), "Say |‹**›bold‹**› now");
	typist.type_script("xx"); // Stays outside, however much is typed.
	assert_eq!(shown(&typist), "Say xx|‹**›bold‹**› now");
	typist.type_script("→y"); // Just inside it.
	assert_eq!(shown(&typist), "Say xx‹**›y|bold‹**› now");
	caret_at(&mut typist, 11);
	typist.type_script("←"); // Just inside the closing `**`.
	typist.type_script("!!"); // Stays inside.
	assert_eq!(shown(&typist), "Say xx‹**›ybold!!|‹**› now");
	typist.type_script("→z"); // Then just outside it.

	assert_eq!(markdown(&typist), "Say xx**ybold!!**z now");
}

#[test]
fn backspace_over_bold_syntax_turns_it_italic_then_plain() {
	let mut typist = from_markdown("Say **bold** now");
	caret_at(&mut typist, 4); // Just inside the opening `**`.
	assert_eq!(shown(&typist), "Say ‹**›|bold‹**› now");

	typist.type_script("⌫");
	assert_eq!(shown(&typist), "Say ‹*›|bold‹*› now");
	assert_eq!(markdown(&typist), "Say *bold* now");

	typist.type_script("⌫");
	assert_eq!(shown(&typist), "Say |bold now");
	assert_eq!(markdown(&typist), "Say bold now");
}

#[test]
fn delete_and_backspace_edit_closing_syntax_too() {
	let mut typist = from_markdown("Say ~~gone~~ and `code` now");

	caret_at(&mut typist, 8);
	typist.type_script("←"); // Just inside the closing `~~`.
	assert_eq!(shown(&typist), "Say ‹~~›gone|‹~~› and code now");
	typist.type_script("⌦");
	assert_eq!(markdown(&typist), "Say gone and `code` now");

	caret_at(&mut typist, 17); // Just after the closing backtick.
	assert_eq!(shown(&typist), "Say gone and ‹`›code‹`›| now");
	typist.type_script("⌫");
	assert_eq!(markdown(&typist), "Say gone and code now");
}

#[test]
fn nested_syntax_opens_outermost_first_and_closes_innermost_first() {
	let mut typist = from_markdown("A **bold *both* bold** z");

	caret_at(&mut typist, 9);
	assert_eq!(shown(&typist), "A ‹**›bold ‹*›bo|th‹*› bold‹**› z");
}

#[test]
fn links_show_their_address() {
	let mut typist = from_markdown("See [the site](https://canvist.dev) now");

	caret_at(&mut typist, 6);
	assert_eq!(
		shown(&typist),
		"See ‹[›th|e site‹](https://canvist.dev)› now"
	);
}

#[test]
fn a_heading_shows_its_level_and_backspace_lowers_it() {
	let mut typist = from_markdown("## Title");

	caret_at(&mut typist, 0);
	assert_eq!(shown(&typist), "‹## ›|Title");
	typist.type_script("⌫");
	assert_eq!(shown(&typist), "‹# ›|Title");
	typist.type_script("⌫");
	assert_eq!(shown(&typist), "|Title");
	assert_eq!(markdown(&typist), "Title");
}

#[test]
fn the_document_never_holds_the_syntax() {
	let mut typist = from_markdown("Say **bold** now");

	caret_at(&mut typist, 6);
	assert!(!typist.editor.adornments().is_empty());
	assert_eq!(markdown(&typist), "Say **bold** now");
	let text = typist
		.editor
		.state()
		.doc()
		.inline(first_block(&typist))
		.expect("text")
		.as_str()
		.to_owned();
	assert_eq!(text, "Say bold now");
}

#[test]
fn formatting_at_the_caret_keeps_its_side_and_its_marks() {
	let mut typist = from_markdown("Say **bold** now");

	caret_at(&mut typist, 8);
	typist.type_script("←"); // Just inside the closing `**`.
	typist.type_script("[Ctrl-i]x"); // Italic, still inside the bold.
	assert_eq!(markdown(&typist), "Say **bold*x*** now");
}
