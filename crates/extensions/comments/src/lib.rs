#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_comments`
//!
//! Comments for the canvist editor: ranges of text tied to discussion
//! threads that live outside the document, anchored as TipTap's Comments
//! and Remirror's annotations anchor them. [`Comments`] adds a `comment` mark
//! whose `threads` attribute lists thread ids, the `setComment` and
//! `unsetComment` commands, and the `activeComments` and `comments` queries.
//! The threads themselves, their messages, authors, and resolution, belong
//! to the host.
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_core::command;
//! use canvist_core::commands;
//! use canvist_extension_comments::Comments;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let comments = builder.add(Comments);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! editor.insert_text("Ship it", &mut env)?;
//! editor.run(&command(commands::select_all), &mut env)?;
//! editor.run(&comments.set("thread-1"), &mut env)?;
//!
//! let threads = comments.threads(editor.state());
//! assert_eq!(threads, [("thread-1".to_owned(), "Ship it".to_owned())]);
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
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::QuerySpec;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::ValueType;
use canvist_core::command;
use canvist_core::commands;
use canvist_core::mark_type;
use canvist_core::position;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::Document;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::MarkSpec;
use canvist_model::MarkTypeId;
use canvist_model::RunRef;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Selection;
use canvist_model::Value;

const COMMENT: &str = "comment";
const THREADS: &str = "threads";

/// Comments, as TipTap's Comments and Remirror's annotations anchor them: a
/// `comment` mark whose `threads` attribute lists the ids of the threads on
/// that text. One mark holding a list lets comments overlap, as one text
/// run can only carry each mark once.
///
/// `setComment` adds a thread to the selected text, `unsetComment` removes
/// it everywhere, and `activeComments` lists the threads at the caret. The
/// threads themselves, their messages, authors, and resolution, belong to
/// the host. The theme tints commented text; HTML writes
/// `<span data-type="comment" data-threads="…">`, and Markdown drops the
/// mark.
#[derive(Clone, Copy, Debug, Default)]
pub struct Comments;

/// Typed access to [`Comments`].
#[derive(Clone, Copy, Debug)]
pub struct CommentsHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl CommentsHandle {
	/// Add thread `thread` to the selected text.
	#[must_use]
	pub fn set(self, thread: &str) -> Command {
		set(thread)
	}

	/// Remove thread `thread` from the whole document.
	#[must_use]
	pub fn unset(self, thread: &str) -> Command {
		unset(thread)
	}

	/// The threads on the text at the selection head.
	#[must_use]
	pub fn active(self, state: &EditorState) -> Vec<String> {
		active(state)
	}

	/// Every thread in the document, in the order it first appears, with
	/// the text it covers.
	#[must_use]
	pub fn threads(self, state: &EditorState) -> Vec<(String, String)> {
		threads(state)
	}
}

/// The thread ids in a comment mark's attributes.
fn ids(attrs: Option<&Attrs>) -> Vec<String> {
	attrs
		.and_then(|attrs| attrs.get(THREADS))
		.and_then(Value::as_array)
		.unwrap_or_default()
		.iter()
		.filter_map(Value::as_str)
		.map(str::to_owned)
		.collect()
}

fn with_ids(ids: &[String]) -> Option<Attrs> {
	if ids.is_empty() {
		return None;
	}

	let values: Vec<Value> = ids.iter().map(|id| Value::from(id.as_str())).collect();
	Some(Attrs::new().with(THREADS, values))
}

/// Add `thread` to every selected character, keeping the threads already
/// there. A caret, or a selection that already carries `thread`
/// everywhere, changes nothing.
fn set(thread: &str) -> Command {
	let thread = thread.to_owned();

	command(move |state| {
		if state.selection().is_caret() {
			return Ok(None);
		}

		let kind = mark_type(state.schema(), COMMENT, COMMENT)?;

		if covers(state, kind, &thread) {
			return Ok(None);
		}

		commands::update_mark_attrs(state, kind, &|current| {
			let mut ids = ids(current);

			if !ids.contains(&thread) {
				ids.push(thread.clone());
			}

			with_ids(&ids)
		})
	})
}

/// Whether every selected character already carries `thread`, so adding it
/// would change nothing.
fn covers(state: &EditorState, kind: MarkTypeId, thread: &str) -> bool {
	let doc = state.doc();

	position::selected_text(doc, state.selection())
		.into_iter()
		.filter(|(_, range)| !range.is_empty())
		.all(|(block, range)| {
			doc.inline(block)
				.and_then(|text| text.mark_runs(range).ok())
				.is_some_and(|runs| {
					runs.iter().all(|(_, marks)| {
						ids(marks.get(kind).map(|mark| &mark.attrs))
							.iter()
							.any(|id| id == thread)
					})
				})
		})
}

fn unset(thread: &str) -> Command {
	let thread = thread.to_owned();

	command(move |state| remove_everywhere(state, &thread))
}

/// Every text run in `doc`, with its block and its threads.
fn runs_with_threads(
	doc: &Document,
	kind: MarkTypeId,
) -> impl Iterator<Item = (BlockId, RunRef<'_>, Vec<String>)> {
	doc.text_blocks().flat_map(move |block| {
		block
			.inline()
			.into_iter()
			.flat_map(InlineText::runs)
			.map(move |run| {
				let threads = ids(run.marks.get(kind).map(|mark| &mark.attrs));
				(block.id(), run, threads)
			})
	})
}

/// Remove `thread` from every comment mark in the document.
fn remove_everywhere(state: &mut EditorState, thread: &str) -> CommandResult {
	let kind = mark_type(state.schema(), COMMENT, COMMENT)?;
	let edits: Vec<_> = runs_with_threads(state.doc(), kind)
		.filter(|(_, _, ids)| ids.iter().any(|id| id == thread))
		.map(|(block, run, mut ids)| {
			ids.retain(|id| id != thread);
			(block, run.range, with_ids(&ids))
		})
		.collect();

	if edits.is_empty() {
		return Ok(None);
	}

	state.change(EditKind::Formatting, |tr| {
		for (block, range, attrs) in edits {
			match attrs {
				Some(attrs) => tr.add_mark(block, range, Mark::with_attrs(kind, attrs))?,
				None => tr.remove_mark(block, range, kind)?,
			}
		}

		Ok(true)
	})
}

/// The threads on the character before the selection head, or the one
/// after it at a block's start.
fn active(state: &EditorState) -> Vec<String> {
	let Some(kind) = state.schema().mark_type(COMMENT) else {
		return Vec::new();
	};
	let Selection::Text { head, .. } = state.selection() else {
		return Vec::new();
	};
	let Some(text) = state.doc().inline(head.block) else {
		return Vec::new();
	};

	let source = text.as_str();
	let offset = (head.offset as usize).min(source.len());
	let before = source[..offset].chars().next_back();

	let range = match before {
		Some(character) => (offset - character.len_utf8())..offset,
		None => 0..source.chars().next().map_or(0, char::len_utf8),
	};

	let marks = text
		.mark_runs(range.start as u32..range.end as u32)
		.ok()
		.and_then(|runs| runs.into_iter().next())
		.map(|(_, marks)| marks);

	ids(marks
		.as_ref()
		.and_then(|marks| marks.get(kind))
		.map(|mark| &mark.attrs))
}

/// Every thread in the document, in order of first appearance, with the
/// text it covers joined across runs and blocks. Embeds such as mentions
/// add no text.
fn threads(state: &EditorState) -> Vec<(String, String)> {
	let Some(kind) = state.schema().mark_type(COMMENT) else {
		return Vec::new();
	};
	let mut found: Vec<(String, String)> = Vec::new();

	for (_, run, threads) in runs_with_threads(state.doc(), kind) {
		let text = if run.embed.is_some() { "" } else { run.text };

		for id in threads {
			cover(&mut found, id, text);
		}
	}

	found
}

/// Add `text` to what thread `id` covers, listing the thread if it is new.
fn cover(found: &mut Vec<(String, String)>, id: String, text: &str) {
	match found.iter_mut().find(|(known, _)| *known == id) {
		Some((_, covered)) => covered.push_str(text),
		None => found.push((id, text.to_owned())),
	}
}

fn thread_field() -> Field {
	Field::required(
		"threadId",
		ValueType::String,
		"The thread's id, from the host.",
	)
}

impl Extension for Comments {
	fn name(&self) -> &'static str {
		"comments"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark(
			MarkSpec::new(COMMENT)
				.inclusive(false)
				.attrs(Attrs::new().with(THREADS, Vec::<Value>::new())),
		)
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Mark, COMMENT).attr(Field::optional(
			THREADS,
			ValueType::list(ValueType::String),
			Value::from(Vec::<Value>::new()),
			"The ids of the threads on this text.",
		))]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(vec![
			CommandSpec::new("setComment", "Add a thread to the selected text.", |args| {
				Ok(set(args.string("threadId")?))
			})
			.arg(thread_field()),
			CommandSpec::new(
				"unsetComment",
				"Remove a thread from the whole document.",
				|args| Ok(unset(args.string("threadId")?)),
			)
			.arg(thread_field()),
		])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(vec![
			QuerySpec::new(
				"activeComments",
				"The threads on the text at the selection head.",
				ValueType::list(ValueType::String),
				|state, _| {
					let ids: Vec<Value> = active(state).into_iter().map(Value::from).collect();
					Ok(Value::from(ids))
				},
			),
			QuerySpec::new(
				"comments",
				"Every thread in the document, in the order it first appears, with the text it \
				 covers.",
				ValueType::list(ValueType::Record(vec![
					Field::required("threadId", ValueType::String, "The thread's id."),
					Field::required("text", ValueType::String, "The text it covers."),
				])),
				|state, _| {
					let list: Vec<Value> = threads(state)
						.into_iter()
						.map(|(id, text)| {
							Value::from(Attrs::new().with("threadId", id).with("text", text))
						})
						.collect();
					Ok(Value::from(list))
				},
			),
		])
	}
}

impl TypedExtension for Comments {
	type Handle = CommentsHandle;

	fn handle(&self, registration: Registration) -> CommentsHandle {
		CommentsHandle {
			_registration: registration,
		}
	}
}

#[cfg(test)]
mod tests {
	use std::time::Duration;

	use canvist_core::Editor;
	use canvist_core::Env;
	use canvist_core::query;
	use canvist_extension_document::Document;
	use canvist_extension_paragraph::Paragraph;
	use canvist_model::Point;

	use super::*;

	/// An editor holding `text` in one paragraph.
	fn commented(text: &str) -> (Editor, CommentsHandle) {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		let comments = builder.add(Comments);
		let mut editor = builder.build().expect("valid editor");
		let mut env = Env::headless(Duration::ZERO);

		editor.insert_text(text, &mut env).expect("typed");

		(editor, comments)
	}

	/// The point before the first `needle` in the paragraph, or after it.
	fn point(editor: &Editor, needle: &str, after: bool) -> Point {
		let doc = editor.state().doc();
		let block = doc.first_text_block().expect("a block");
		let start = doc.plain_text().find(needle).expect("present");
		let offset = if after { start + needle.len() } else { start };

		Point::new(block, u32::try_from(offset).expect("short"))
	}

	/// Select from the start of `start` to the end of `end`.
	fn select(editor: &mut Editor, start: &str, end: &str) {
		let selection = Selection::text(point(editor, start, false), point(editor, end, true));

		editor
			.select(selection, &mut Env::headless(Duration::ZERO))
			.expect("selected");
	}

	/// Put the caret just before `before`.
	fn caret(editor: &mut Editor, before: &str) {
		let selection = Selection::caret(point(editor, before, false));

		editor
			.select(selection, &mut Env::headless(Duration::ZERO))
			.expect("selected");
	}

	fn run(editor: &mut Editor, command: &Command) -> bool {
		let mut env = Env::headless(Duration::ZERO);
		editor.run(command, &mut env).expect("ran").is_some()
	}

	#[test]
	fn overlapping_threads_share_text() {
		let (mut editor, comments) = commented("one two three");

		select(&mut editor, "one", "two");
		assert!(run(&mut editor, &comments.set("a")));
		select(&mut editor, "two", "three");
		assert!(run(&mut editor, &comments.set("b")));

		caret(&mut editor, "wo");
		assert_eq!(comments.active(editor.state()), ["a", "b"]);
		assert_eq!(
			comments.threads(editor.state()),
			[
				("a".to_owned(), "one two".to_owned()),
				("b".to_owned(), "two three".to_owned()),
			]
		);
	}

	#[test]
	fn setting_a_thread_twice_or_at_a_caret_changes_nothing() {
		let (mut editor, comments) = commented("text");

		select(&mut editor, "text", "text");
		assert!(run(&mut editor, &comments.set("a")));
		assert!(!run(&mut editor, &comments.set("a")));

		caret(&mut editor, "xt");
		assert!(!run(&mut editor, &comments.set("b")));
	}

	#[test]
	fn unset_removes_one_thread_and_keeps_the_others() {
		let (mut editor, comments) = commented("text");

		select(&mut editor, "text", "text");
		run(&mut editor, &comments.set("a"));
		run(&mut editor, &comments.set("b"));

		assert!(run(&mut editor, &comments.unset("a")));
		assert_eq!(comments.active(editor.state()), ["b"]);

		assert!(run(&mut editor, &comments.unset("b")));
		assert!(!query::mark_active(editor.state(), COMMENT));
		assert!(!run(&mut editor, &comments.unset("b")));
	}

	#[test]
	fn the_caret_at_a_block_start_reads_the_next_character() {
		let (mut editor, comments) = commented("text");

		select(&mut editor, "t", "t");
		run(&mut editor, &comments.set("a"));
		caret(&mut editor, "text");

		assert_eq!(comments.active(editor.state()), ["a"]);
	}
}
