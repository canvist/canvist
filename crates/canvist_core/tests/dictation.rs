//! Dictation: provisional text beside the document, final text inserted in
//! order, one undo step each.

mod support;

use std::time::Duration;

use canvist_core::EditKind;
use canvist_core::Editor;
use canvist_core::Env;
use canvist_core::Extension;
use canvist_core::Transcript;
use canvist_model::Point;
use canvist_model::SchemaBuilder;
use support::render;

/// When the speech model reports, in milliseconds on the editor clock.
const SPOKEN: u64 = 20;

/// When the person does something else afterwards.
const LATER: u64 = 50;

struct Kit;

impl Extension for Kit {
	fn name(&self) -> &'static str {
		"kit"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		support::specs(schema)
	}
}

fn editor(outline: &str) -> Editor {
	let mut selection = None;
	let mut editor = Editor::builder()
		.extension(Kit)
		.build_with(|schema, _client| {
			let (doc, marked) = support::parse(&schema, outline);
			selection = marked;
			Ok(doc)
		})
		.expect("editor builds");

	if let Some(selection) = selection {
		editor.select(selection, &mut at(0)).expect("select");
	}

	editor
}

fn at(millis: u64) -> Env<'static> {
	Env::headless(Duration::from_millis(millis))
}

/// The provisional text at the caret, if any.
fn provisional(editor: &Editor) -> Option<String> {
	let state = editor.state();
	let caret = state.selection().as_caret()?;
	state.dictation().provisional(state.doc(), caret)
}

fn partial(editor: &mut Editor, segment: u32, text: &str) {
	editor
		.dictate(segment, Transcript::Partial(text), &mut at(SPOKEN))
		.expect("partial");
}

fn finish(editor: &mut Editor, segment: u32, text: &str) {
	editor
		.dictate(segment, Transcript::Final(text), &mut at(SPOKEN))
		.expect("final");
}

#[test]
fn provisional_text_stays_out_of_the_document() {
	let mut editor = editor("paragraph: Hello|");
	let version = editor.state().doc().version();

	partial(&mut editor, 0, "the meeting is on tuesday");
	partial(&mut editor, 0, "the meeting is on Tuesday no Wednesday");

	assert_eq!(render(editor.state()), "paragraph: Hello|");
	assert_eq!(editor.state().doc().version(), version);
	assert_eq!(
		provisional(&editor).as_deref(),
		Some(" the meeting is on Tuesday no Wednesday")
	);
	assert!(!editor.can_undo());
}

#[test]
fn a_final_segment_is_inserted_at_the_caret_as_one_undo_step() {
	let mut editor = editor("paragraph: Hello|");
	partial(&mut editor, 0, "world");
	finish(&mut editor, 0, "world.");

	assert_eq!(render(editor.state()), "paragraph: Hello world.|");
	assert_eq!(provisional(&editor), None);

	editor.undo(&mut at(LATER)).expect("undo");
	assert_eq!(render(editor.state()), "paragraph: Hello|");
}

#[test]
fn each_final_segment_is_its_own_undo_step() {
	let mut editor = editor("paragraph: |");
	finish(&mut editor, 0, "One.");
	finish(&mut editor, 1, "Two.");

	assert_eq!(render(editor.state()), "paragraph: One. Two.|");

	let update = editor.undo(&mut at(LATER)).expect("undo").expect("undone");
	assert_eq!(update.kind, EditKind::Dictation);
	assert_eq!(render(editor.state()), "paragraph: One.|");
}

#[test]
fn later_segments_wait_for_earlier_ones() {
	let mut editor = editor("paragraph: |");
	partial(&mut editor, 0, "first");
	finish(&mut editor, 1, "Second.");

	assert_eq!(render(editor.state()), "paragraph: |");
	assert_eq!(provisional(&editor).as_deref(), Some("first Second."));

	finish(&mut editor, 0, "First.");
	assert_eq!(render(editor.state()), "paragraph: First. Second.|");
}

#[test]
fn segments_join_without_spaces_before_punctuation_or_after_openers() {
	let mut editor = editor("paragraph: (|");
	finish(&mut editor, 0, "aside");
	finish(&mut editor, 1, ", really");

	assert_eq!(render(editor.state()), "paragraph: (aside, really|");
}

#[test]
fn text_dictated_mid_sentence_keeps_the_next_word_apart() {
	let mut editor = editor("paragraph: Hello |world");
	partial(&mut editor, 0, "big");
	assert_eq!(provisional(&editor).as_deref(), Some("big "));

	finish(&mut editor, 0, "big");
	assert_eq!(render(editor.state()), "paragraph: Hello big| world");

	finish(&mut editor, 1, "wide");
	assert_eq!(render(editor.state()), "paragraph: Hello big wide| world");
}

#[test]
fn speech_replaces_the_selection_as_soon_as_it_starts() {
	let mut editor = editor("paragraph: The ^Tuesday| meeting");
	partial(&mut editor, 0, "Wednesday");

	assert_eq!(render(editor.state()), "paragraph: The | meeting");
	assert_eq!(provisional(&editor).as_deref(), Some("Wednesday"));

	finish(&mut editor, 0, "Wednesday");
	assert_eq!(render(editor.state()), "paragraph: The Wednesday| meeting");
}

#[test]
fn white_space_and_line_breaks_collapse_into_single_spaces() {
	let mut editor = editor("paragraph: |");
	finish(&mut editor, 0, "  one\n\ntwo  ");

	assert_eq!(render(editor.state()), "paragraph: one two|");
}

#[test]
fn an_empty_final_segment_inserts_nothing_and_lets_the_next_through() {
	let mut editor = editor("paragraph: |");
	finish(&mut editor, 0, "   ");
	finish(&mut editor, 1, "Next.");

	assert_eq!(render(editor.state()), "paragraph: Next.|");
}

#[test]
fn news_of_an_inserted_segment_is_ignored() {
	let mut editor = editor("paragraph: |");
	finish(&mut editor, 0, "Done.");
	partial(&mut editor, 0, "done and dusted");

	assert_eq!(render(editor.state()), "paragraph: Done.|");
	assert_eq!(provisional(&editor), None);
}

#[test]
fn stopping_forgets_provisional_text_and_restarts_numbering() {
	let mut editor = editor("paragraph: |");
	finish(&mut editor, 0, "Kept.");
	partial(&mut editor, 1, "dropped");

	assert!(editor.stop_dictation());
	assert_eq!(provisional(&editor), None);
	assert!(!editor.stop_dictation());

	finish(&mut editor, 0, "Again.");
	assert_eq!(render(editor.state()), "paragraph: Kept. Again.|");
}

#[test]
fn inserted_text_is_remembered_with_when_it_settled() {
	let mut editor = editor("paragraph: Hi|");
	finish(&mut editor, 0, "there.");

	let settling = editor.state().dictation().settling();
	assert_eq!(settling.len(), 1);
	assert_eq!((settling[0].start, settling[0].end), (3, 9));
	assert_eq!(settling[0].since, Duration::from_millis(SPOKEN));
}

#[test]
fn settling_text_follows_edits_before_it() {
	let mut editor = editor("paragraph: Hi|");
	finish(&mut editor, 0, "there.");

	let block = editor.state().selection().as_caret().expect("caret").block;
	editor
		.select(
			canvist_model::Selection::caret(Point::new(block, 0)),
			&mut at(LATER),
		)
		.expect("select");
	editor.insert_text("Oh, ", &mut at(LATER)).expect("type");

	let settling = editor.state().dictation().settling()[0];
	assert_eq!((settling.start, settling.end), (7, 13));
}

#[test]
fn deleting_settling_text_forgets_it() {
	let mut editor = editor("paragraph: |");
	finish(&mut editor, 0, "Gone.");
	editor.undo(&mut at(LATER)).expect("undo");

	assert!(editor.state().dictation().settling().is_empty());
}
