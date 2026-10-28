//! Dictation as the iOS and Android views drive it: provisional text beside
//! the document, final text inserted as its own undo step, and frames that
//! animate only when the view asks.

use std::sync::Arc;
use std::sync::Mutex;

use canvist_ffi::CanvistError;
use canvist_ffi::DisplayItem;
use canvist_ffi::DocFormat;
use canvist_ffi::Editor;
use canvist_ffi::EditorChange;
use canvist_ffi::EditorObserver;
use canvist_ffi::EditorOptions;
use canvist_ffi::Frame;
use canvist_ffi::testing::FixedPlatformMeasurer;

type Result = std::result::Result<(), CanvistError>;

/// The alpha of provisional text with animations off: 45% of opaque.
const FAINT: u8 = 115;

fn editor(markdown: &str) -> Arc<Editor> {
	let editor = Editor::new(
		EditorOptions::starter_kit(),
		Arc::new(FixedPlatformMeasurer::default()),
	)
	.expect("the StarterKit is valid");
	editor
		.load(DocFormat::Markdown, markdown)
		.expect("valid markdown");
	editor.set_viewport(400.0, 600.0);
	editor
}

/// An editor holding `markdown` with the caret at the end of its text.
fn at_end(markdown: &str) -> Arc<Editor> {
	let editor = editor(markdown);
	let end = editor.text_length();
	editor.select(end, end).expect("select the end");
	editor
}

/// Each text run's text and alpha, in paint order.
fn runs(frame: &Frame) -> Vec<(String, u8)> {
	frame
		.items
		.iter()
		.flatten()
		.filter_map(|item| {
			match item {
				DisplayItem::Text { text, color, .. } => Some((text.clone(), color.a)),
				_ => None,
			}
		})
		.collect()
}

#[derive(Default)]
struct Recorder {
	changes: Mutex<Vec<EditorChange>>,
}

impl EditorObserver for Recorder {
	fn changed(&self, change: EditorChange) {
		self.changes.lock().expect("lock").push(change);
	}
}

#[test]
fn provisional_text_shows_without_changing_the_document() -> Result {
	let editor = at_end("Hello");
	let recorder = Arc::new(Recorder::default());
	editor.add_observer(recorder.clone());

	assert!(editor.dictate(0, "wor", false)?);
	assert!(editor.dictate(0, "world", false)?);
	assert!(!editor.dictate(0, "world", false)?);

	assert_eq!(editor.plain_text(), "Hello");
	assert_eq!(editor.dictation_draft().as_deref(), Some(" world"));
	assert!(!editor.can_undo());

	let changes = recorder.changes.lock().expect("lock");
	assert_eq!(changes.len(), 2);
	assert!(
		changes
			.iter()
			.all(|change| change.dictation && !change.document)
	);
	Ok(())
}

#[test]
fn the_caret_shows_after_provisional_text() -> Result {
	let editor = at_end("Hello");
	let caret = editor.selection().head;
	let before = editor.caret_rect(caret).expect("a caret").x;

	editor.dictate(0, "world", false)?;
	let after = editor.caret_rect(caret).expect("a caret").x;

	assert!(
		after > before,
		"the caret moved past the draft: {before} → {after}"
	);
	Ok(())
}

#[test]
fn final_text_is_inserted_as_one_undo_step() -> Result {
	let editor = at_end("Hello");

	editor.dictate(0, "world", false)?;
	editor.dictate(0, "world.", true)?;

	assert_eq!(editor.plain_text(), "Hello world.");
	assert_eq!(editor.dictation_draft(), None);

	editor.undo()?;
	assert_eq!(editor.plain_text(), "Hello");
	Ok(())
}

#[test]
fn provisional_text_is_painted_faint() -> Result {
	let editor = at_end("Hello");
	editor.dictate(0, "world", false)?;

	let frame = editor.frame()?;
	let faint: Vec<String> = runs(&frame)
		.into_iter()
		.filter(|(_, alpha)| *alpha == FAINT)
		.map(|(text, _)| text)
		.collect();

	assert_eq!(faint.concat(), " world");
	assert!(
		!frame.animated,
		"animations are off until the view drives frames"
	);
	Ok(())
}

#[test]
fn frames_animate_dictation_once_the_view_asks() -> Result {
	let editor = at_end("Hello");
	editor.set_animations(true);

	editor.dictate(0, "world", false)?;
	assert!(editor.frame()?.animated, "the draft breathes");

	editor.dictate(0, "world.", true)?;
	assert!(editor.frame()?.animated, "the final text settles");

	editor.set_animations(false);
	assert!(!editor.frame()?.animated);
	Ok(())
}

#[test]
fn stopping_drops_provisional_text() -> Result {
	let editor = at_end("Hello");
	editor.dictate(0, "world", false)?;

	assert!(editor.stop_dictation());
	assert_eq!(editor.dictation_draft(), None);
	assert!(!editor.stop_dictation());
	assert_eq!(editor.plain_text(), "Hello");
	Ok(())
}
