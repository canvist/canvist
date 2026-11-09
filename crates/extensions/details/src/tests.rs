use std::time::Duration;

use canvist_core::Editor;
use canvist_core::Env;
use canvist_core::KeyPress;
use canvist_extension_paragraph::Paragraph;

use super::*;

/// An atomic `rule` block, to put at the start of a details' content.
struct Rule;

impl Extension for Rule {
	fn name(&self) -> &'static str {
		"rule"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.block(BlockSpec::leaf("rule"))
	}
}

fn env() -> Env<'static> {
	Env::headless(Duration::ZERO)
}

/// An editor with `text` wrapped in an open details, and the caret in
/// its empty summary.
fn wrapped(text: &str) -> Result<(Editor, DetailsHandle), EditorError> {
	let mut builder = Editor::builder()
		.extension(canvist_extension_document::Document)
		.extension(Paragraph)
		.extension(Rule);
	let details = builder.add(Details);
	let mut editor = builder.build()?;
	editor.insert_text(text, &mut env())?;
	editor.run(&details.set(), &mut env())?;
	Ok((editor, details))
}

fn press(editor: &mut Editor, key: &str) -> CommandResult {
	editor.handle_key(KeyPress::plain(key), &mut env())
}

/// The type names of the blocks at the root.
fn root(editor: &Editor) -> Vec<String> {
	let doc = editor.state().doc();
	doc.children(None)
		.iter()
		.filter_map(|&id| doc.block(id))
		.map(|block| doc.schema().block(block.kind()).name.to_string())
		.collect()
}

#[test]
fn enter_in_a_closed_summary_opens_it_and_moves_into_the_content() -> Result<(), EditorError> {
	let (mut editor, details) = wrapped("Hidden")?;
	editor.insert_text("Summary", &mut env())?;
	editor.run(&details.toggle_open(), &mut env())?;
	assert_eq!(details.is_open(editor.state()), Some(false));

	assert!(press(&mut editor, "Enter")?.is_some());
	assert_eq!(details.is_open(editor.state()), Some(true));
	editor.insert_text("> ", &mut env())?;
	assert_eq!(editor.state().doc().plain_text(), "Summary\n> Hidden");
	Ok(())
}

#[test]
fn enter_selects_an_atomic_block_starting_the_content() -> Result<(), EditorError> {
	let (mut editor, _) = wrapped("Hidden")?;
	let summary = editor
		.state()
		.selection()
		.as_caret()
		.expect("a caret")
		.block;
	let rule = editor.schema().block_type("rule").expect("a rule type");
	let content = editor
		.state()
		.doc()
		.children(position::parent_of(editor.state().doc(), summary))[1];
	let add_rule = command(move |state| {
		state.change(EditKind::Structure, |tr| {
			let id = tr.allocate_id()?;
			tr.insert_blocks(Some(content), 0, vec![BlockTree::leaf(id, rule)])?;
			Ok(true)
		})
	});
	editor.run(&add_rule, &mut env())?;

	press(&mut editor, "Enter")?;
	let first = editor.state().doc().children(Some(content))[0];
	assert_eq!(
		editor.state().selection(),
		Selection::block(first),
		"not the start of the document"
	);
	Ok(())
}

#[test]
fn closing_moves_a_caret_out_of_the_hidden_content() -> Result<(), EditorError> {
	let (mut editor, details) = wrapped("Hidden")?;
	editor.insert_text("Summary", &mut env())?;
	press(&mut editor, "Enter")?;
	editor.run(&details.toggle_open(), &mut env())?;

	let caret = editor.state().selection().as_caret().expect("a caret");
	let summary = editor
		.state()
		.doc()
		.inline(caret.block)
		.expect("a text block");
	assert_eq!((summary.as_str(), caret.offset), ("Summary", 7));
	Ok(())
}

#[test]
fn backspace_at_the_start_of_the_summary_unwraps_the_details() -> Result<(), EditorError> {
	let (mut editor, details) = wrapped("Hidden")?;
	editor.insert_text("Summary", &mut env())?;
	let caret = editor.state().selection().as_caret().expect("a caret");
	editor.select(Selection::caret(Point::new(caret.block, 0)), &mut env())?;

	assert!(press(&mut editor, "Backspace")?.is_some());
	assert_eq!(root(&editor), ["paragraph", "paragraph"]);
	assert_eq!(editor.state().doc().plain_text(), "Summary\nHidden");
	assert_eq!(details.is_open(editor.state()), None);
	Ok(())
}

#[test]
fn a_details_missing_its_content_is_repaired() -> Result<(), EditorError> {
	let (mut editor, _) = wrapped("Hidden")?;
	let schema = editor.schema().clone();
	let kind = |name| schema.block_type(name).expect("a block type");
	let (details, summary) = (kind(DETAILS), kind(SUMMARY));
	let add_broken = command(move |state| {
		state.change(EditKind::Structure, |tr| {
			let ids = (tr.allocate_id()?, tr.allocate_id()?);
			let summary = BlockTree::text(ids.1, summary, InlineText::new());
			tr.insert_blocks(
				None,
				0,
				vec![BlockTree::container(ids.0, details, vec![summary])],
			)?;
			Ok(true)
		})
	});
	editor.run(&add_broken, &mut env())?;

	let doc = editor.state().doc();
	let first = doc.children(None)[0];
	let shape: Vec<_> = doc
		.children(Some(first))
		.iter()
		.filter_map(|&id| doc.block(id).map(Block::kind))
		.collect();
	assert_eq!(shape, [summary, kind(CONTENT)]);
	let content = doc.children(Some(first))[1];
	assert_eq!(doc.children(Some(content)).len(), 1, "an empty paragraph");
	Ok(())
}
