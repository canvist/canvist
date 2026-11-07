//! Conditional text: set, toggled, and read by command, kept apart from the
//! text typed after it, and through HTML and Markdown.

#![cfg(feature = "full")]

mod support;

use std::time::Duration;

use canvist_core::Editor;
use canvist_core::EditorError;
use canvist_core::Env;
use canvist_extensions::clipboard;
use canvist_model::Attrs;
use canvist_model::Value;
use serde_json::json;
use support::Typist;

fn env() -> Env<'static> {
	Env::headless(Duration::ZERO)
}

fn typist(script: &str) -> Typist {
	let mut typist = Typist::with(support::editor_with(canvist_extensions::extras()));
	typist.type_script(script);
	typist
}

fn run(editor: &mut Editor, name: &str, when: Option<&str>) -> Result<bool, EditorError> {
	let args = when.map_or_else(Attrs::new, |when| Attrs::new().with("when", when));
	Ok(editor.run_named(name, &args, &mut env())?.is_some())
}

fn when(editor: &Editor) -> Value {
	editor
		.query("conditionalWhen", &Attrs::new())
		.expect("a known query")
}

/// "Thanks " then `handle` selected.
fn selecting(handle: &str) -> Typist {
	let back = "[Shift-ArrowLeft]".repeat(handle.chars().count());
	typist(&format!("Thanks {handle}{back}"))
}

#[test]
fn conditional_text_is_set_toggled_and_read() {
	let mut typist = selecting("@ada");

	assert!(
		run(
			&mut typist.editor,
			"setConditional",
			Some("instagram threads")
		)
		.expect("runs")
	);
	assert_eq!(when(&typist.editor), Value::from("instagram threads"));
	assert_eq!(
		typist
			.editor
			.query("isConditional", &Attrs::new())
			.expect("a query"),
		Value::Bool(true)
	);
	assert_eq!(
		typist.snapshot()["doc"][0]["content"][1],
		json!({
			"type": "text",
			"text": "@ada",
			"marks": [{ "type": "conditional", "attrs": { "when": "instagram threads" } }],
		})
	);

	assert!(run(&mut typist.editor, "toggleConditional", Some("bluesky")).expect("runs"));
	assert_eq!(when(&typist.editor), Value::from("bluesky"));

	assert!(run(&mut typist.editor, "toggleConditional", Some("bluesky")).expect("runs"));
	assert_eq!(when(&typist.editor), Value::Null);
	assert_eq!(
		typist.snapshot()["doc"][0]["content"],
		json!([{ "type": "text", "text": "Thanks @ada" }])
	);
}

#[test]
fn a_conditional_needs_a_when() {
	let mut typist = selecting("@ada");

	assert!(run(&mut typist.editor, "setConditional", None).is_err());
	assert!(run(&mut typist.editor, "setConditional", Some("  ")).is_err());
	assert!(run(&mut typist.editor, "toggleConditional", Some("")).is_err());
	assert_eq!(when(&typist.editor), Value::Null);
}

#[test]
fn unset_makes_the_selection_unconditional() {
	let mut typist = selecting("@ada");

	run(&mut typist.editor, "setConditional", Some("x")).expect("runs");
	assert!(run(&mut typist.editor, "unsetConditional", None).expect("runs"));
	assert_eq!(
		typist
			.editor
			.query("isConditional", &Attrs::new())
			.expect("a query"),
		Value::Bool(false)
	);
}

#[test]
fn text_typed_after_a_conditional_is_not_conditional() {
	let mut typist = selecting("@ada");

	run(&mut typist.editor, "setConditional", Some("bluesky")).expect("runs");
	typist.type_script("[End] and team");

	assert_eq!(
		typist.snapshot()["doc"][0]["content"],
		json!([
			{ "type": "text", "text": "Thanks " },
			{
				"type": "text",
				"text": "@ada",
				"marks": [{ "type": "conditional", "attrs": { "when": "bluesky" } }],
			},
			{ "type": "text", "text": " and team" },
		])
	);
}

#[test]
fn a_caret_takes_inserted_conditional_text_not_a_mark() {
	let mut typist = typist("[Mod-b]Thanks !");
	typist.type_script("[ArrowLeft]");

	assert!(!run(&mut typist.editor, "setConditional", Some("x")).expect("runs"));
	assert!(!run(&mut typist.editor, "toggleConditional", Some("x")).expect("runs"));

	let args = Attrs::new().with("when", "x").with("text", "@ada_x");
	assert!(
		typist
			.editor
			.run_named("insertConditional", &args, &mut env())
			.expect("runs")
			.is_some()
	);
	typist.type_script(" ");

	assert_eq!(
		typist.snapshot()["doc"][0]["content"],
		json!([
			{ "type": "text", "text": "Thanks ", "marks": [{ "type": "bold" }] },
			{
				"type": "text",
				"text": "@ada_x",
				"marks": [{ "type": "bold" }, { "type": "conditional", "attrs": { "when": "x" } }],
			},
			{ "type": "text", "text": " !", "marks": [{ "type": "bold" }] },
		])
	);

	let empty = Attrs::new().with("when", "x").with("text", "");
	assert!(
		typist
			.editor
			.run_named("insertConditional", &empty, &mut env())
			.expect("runs")
			.is_none()
	);
}

#[test]
fn conditional_text_round_trips_through_html_and_is_plain_in_markdown() {
	let typist = typist("");
	let schema = typist.editor.schema();
	let html = "<p>Thanks <span data-type=\"conditional\" \
	            data-when=\"bluesky\">@ada.bsky.social</span><span data-type=\"conditional\" \
	            data-when=\"instagram threads\">@ada_ig</span>!</p>";
	let slice = clipboard::from_html(schema, html);

	assert_eq!(clipboard::to_html(schema, &slice), html);
	assert_eq!(
		clipboard::to_markdown(schema, &slice),
		"Thanks @ada.bsky.social@ada\\_ig!"
	);
}

#[test]
fn html_without_a_when_is_plain_text() {
	let typist = typist("");
	let schema = typist.editor.schema();
	let slice = clipboard::from_html(
		schema,
		"<p><span data-type=\"conditional\" data-when=\" \">hi</span> <span \
		 data-type=\"conditional\">there</span></p>",
	);

	assert_eq!(clipboard::to_html(schema, &slice), "<p>hi there</p>");
}

#[test]
fn html_escapes_the_when() {
	let typist = typist("");
	let schema = typist.editor.schema();
	let html = "<p><span data-type=\"conditional\" data-when=\"a&quot;&gt;&lt;b\">hi</span></p>";
	let slice = clipboard::from_html(schema, html);

	assert_eq!(clipboard::to_html(schema, &slice), html);
}
