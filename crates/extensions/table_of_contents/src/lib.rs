#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_table_of_contents`
//!
//! A table of contents for the canvist editor: the document's headings as a
//! numbered outline, as in TipTap's `@tiptap/extension-table-of-contents`.
//! [`TableOfContents`] adds the `tableOfContents` query, which lists every
//! `heading` with its level, text, outline number, and whether the selection
//! is under it, and the `goToHeading` command, which puts the caret at the
//! start of one. It adds no nodes or marks: register it with the `heading`
//! extension.
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_heading::Heading;
//! use canvist_extension_paragraph::Paragraph;
//! use canvist_extension_table_of_contents::TableOfContents;
//!
//! let mut builder = Editor::builder()
//! 	.extension(Document)
//! 	.extension(Paragraph)
//! 	.extension(Heading::default());
//! let toc = builder.add(TableOfContents);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! for character in "## Intro".chars() {
//! 	editor.insert_text(&character.to_string(), &mut env)?;
//! }
//!
//! let entries = toc.entries(editor.state());
//! assert_eq!(entries.len(), 1);
//! assert_eq!((entries[0].text.as_str(), entries[0].number.as_str()), ("Intro", "1"));
//! assert!(entries[0].active);
//! # Ok::<(), canvist_core::EditorError>(())
//! ```
use canvist_core::Command;
use canvist_core::CommandResult;
use canvist_core::CommandSpec;
use canvist_core::EditKind;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::Field;
use canvist_core::QuerySpec;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::ValueType;
use canvist_core::command;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::EMBED_CHAR;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::Selection;
use canvist_model::Value;

const HEADING: &str = "heading";

/// TipTap's `TableOfContents`: the headings of the document, in order, with
/// their levels, text, and outline numbers.
///
/// The `tableOfContents` query lists them, marking the heading the
/// selection is under as active, and `goToHeading` puts the caret at the
/// start of one so the host can scroll it into view. Numbers follow the
/// outline rather than the levels: a level 3 heading straight under a level
/// 1 heading is `1.1`, as TipTap numbers them.
#[derive(Clone, Copy, Debug, Default)]
pub struct TableOfContents;

/// One heading in the table of contents.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TocEntry {
	/// The heading block.
	pub block: BlockId,
	/// Its level, from 1.
	pub level: i64,
	/// Its plain text.
	pub text: String,
	/// Its outline number, such as `2.1`.
	pub number: String,
	/// Whether the selection is under it: in it, or after it and before the
	/// next heading.
	pub active: bool,
}

/// Typed access to [`TableOfContents`].
#[derive(Clone, Copy, Debug)]
pub struct TableOfContentsHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl TableOfContentsHandle {
	/// Every heading, in document order.
	#[must_use]
	pub fn entries(self, state: &EditorState) -> Vec<TocEntry> {
		entries(state)
	}

	/// Put the caret at the start of heading `index`.
	#[must_use]
	pub fn go_to(self, index: usize) -> Command {
		command(move |state| go_to(state, index))
	}
}

/// The outline number of the next heading of `level`, given the open
/// headings above it as `(level, ordinal)` pairs.
fn number(open: &mut Vec<(i64, u32)>, level: i64) -> String {
	let mut closed = None;

	while let Some(&(top, ordinal)) = open.last() {
		if top <= level {
			break;
		}

		closed = Some(ordinal);
		open.pop();
	}

	match open.last_mut() {
		Some((top, ordinal)) if *top == level => *ordinal += 1,
		_ => open.push((level, closed.map_or(1, |ordinal| ordinal + 1))),
	}

	open.iter()
		.map(|(_, ordinal)| ordinal.to_string())
		.collect::<Vec<_>>()
		.join(".")
}

/// Every heading in document order, numbered by outline, with the one the
/// selection head is under marked active.
fn entries(state: &EditorState) -> Vec<TocEntry> {
	let doc = state.doc();
	let Some(heading) = doc.schema().block_type(HEADING) else {
		return Vec::new();
	};

	let head = match state.selection() {
		Selection::Text { head, .. } => head.block,
		Selection::Blocks { head, .. } => head,
	};

	let mut open = Vec::new();
	let mut found = Vec::new();
	let mut active = None;

	for block in doc.iter() {
		if block.kind() == heading {
			let level = block
				.attrs()
				.get("level")
				.and_then(Value::as_int)
				.unwrap_or(1);
			let text = block
				.inline()
				.map(|inline| inline.as_str().replace(EMBED_CHAR, ""))
				.unwrap_or_default();
			found.push(TocEntry {
				block: block.id(),
				level,
				number: number(&mut open, level),
				text,
				active: false,
			});
		}

		// The head block itself, not its outermost container: a caret in a
		// heading inside a quote is under that heading, not the one before
		// the quote.
		if block.id() == head {
			active = found.len().checked_sub(1);
		}
	}

	if let Some(entry) = active.and_then(|index| found.get_mut(index)) {
		entry.active = true;
	}

	found
}

fn go_to(state: &mut EditorState, index: usize) -> CommandResult {
	let Some(entry) = entries(state).into_iter().nth(index) else {
		return Ok(None);
	};

	state.change(EditKind::Selection, |tr| {
		tr.set_selection(Selection::caret(Point::new(entry.block, 0)));
		Ok(true)
	})
}

fn entries_value(state: &EditorState) -> Value {
	let list: Vec<Value> = entries(state)
		.into_iter()
		.map(|entry| {
			Value::from(
				Attrs::new()
					.with("level", entry.level)
					.with("text", entry.text)
					.with("number", entry.number)
					.with("active", entry.active),
			)
		})
		.collect();

	Value::from(list)
}

impl Extension for TableOfContents {
	fn name(&self) -> &'static str {
		"tableOfContents"
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let go_to_heading = CommandSpec::new(
			"goToHeading",
			"Put the caret at the start of a heading, by its place in the table of contents.",
			|args| {
				let index = usize::try_from(args.int("index")?).map_err(|_| {
					EditorError::InvalidArgs {
						name: "goToHeading".to_owned(),
						reason: "`index` must not be negative".to_owned(),
					}
				})?;
				Ok(command(move |state| go_to(state, index)))
			},
		)
		.arg(Field::required(
			"index",
			ValueType::Int,
			"The heading's place in the table of contents, from zero.",
		));

		Ok(vec![go_to_heading])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(vec![QuerySpec::new(
			"tableOfContents",
			"The document's headings in order, with their levels, text, outline numbers, and \
			 which one the selection is under.",
			ValueType::list(ValueType::Record(vec![
				Field::required("level", ValueType::Int, "The heading level, from 1."),
				Field::required("text", ValueType::String, "The heading's text."),
				Field::required(
					"number",
					ValueType::String,
					"Its outline number, such as `2.1`.",
				),
				Field::required(
					"active",
					ValueType::Bool,
					"Whether the selection is under it.",
				),
			])),
			|state, _| Ok(entries_value(state)),
		)])
	}
}

impl TypedExtension for TableOfContents {
	type Handle = TableOfContentsHandle;

	fn handle(&self, registration: Registration) -> TableOfContentsHandle {
		TableOfContentsHandle {
			_registration: registration,
		}
	}
}

#[cfg(test)]
mod tests {
	use std::time::Duration;

	use canvist_core::Editor;
	use canvist_core::Env;
	use canvist_core::commands;
	use canvist_extension_document::Document;
	use canvist_extension_heading::Heading;
	use canvist_extension_paragraph::Paragraph;

	use super::*;

	/// The outline numbers of headings at `levels`, in order.
	fn numbers(levels: &[i64]) -> Vec<String> {
		let mut open = Vec::new();
		levels
			.iter()
			.map(|&level| number(&mut open, level))
			.collect()
	}

	/// Type `character`, or start a new block for a newline.
	fn type_character(editor: &mut Editor, character: char, env: &mut Env<'_>) -> CommandResult {
		if character == '\n' {
			return editor.run(&command(commands::split_block), env);
		}

		editor.insert_text(&character.to_string(), env)
	}

	/// A block per line of `script`, typed a character at a time so input
	/// rules fire, with the caret at the end of the last.
	fn typed(script: &[&str]) -> (Editor, TableOfContentsHandle) {
		let mut builder = Editor::builder()
			.extension(Document)
			.extension(Paragraph)
			.extension(Heading::default());
		let toc = builder.add(TableOfContents);
		let mut editor = builder.build().expect("valid editor");
		let mut env = Env::headless(Duration::ZERO);
		let typing = script.join("\n");

		for character in typing.chars() {
			type_character(&mut editor, character, &mut env).expect("typed");
		}

		(editor, toc)
	}

	#[test]
	fn numbers_follow_the_outline() {
		let expected = ["1", "1.1", "1.2", "1.3", "2", "2.1"];

		assert_eq!(numbers(&[1, 3, 2, 2, 1, 2]), expected);
	}

	#[test]
	fn closing_several_levels_continues_the_one_left_open() {
		let expected = ["1", "1.1", "1.1.1", "1.2", "2"];

		assert_eq!(numbers(&[1, 2, 3, 2, 1]), expected);
	}

	#[test]
	fn the_heading_above_the_caret_is_active() {
		let (editor, toc) = typed(&["# One", "## Two", "text"]);
		let active: Vec<bool> = toc
			.entries(editor.state())
			.iter()
			.map(|entry| entry.active)
			.collect();

		assert_eq!(active, [false, true]);
	}

	#[test]
	fn go_to_moves_the_caret_and_ignores_missing_headings() {
		let (mut editor, toc) = typed(&["# One", "## Two", "text"]);
		let mut env = Env::headless(Duration::ZERO);

		assert!(editor.run(&toc.go_to(0), &mut env).expect("ran").is_some());
		assert!(toc.entries(editor.state())[0].active);
		assert!(editor.run(&toc.go_to(2), &mut env).expect("ran").is_none());
	}
}
