#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_drag_handle`
//!
//! Moving blocks among their siblings for the canvist editor: what a drag
//! handle, or Alt and an arrow key, does. [`DragHandle`] adds the commands
//! behind TipTap's `@tiptap/extension-drag-handle`: `moveBlockUp` and
//! `moveBlockDown`, bound to `Alt-ArrowUp` and `Alt-ArrowDown`, and
//! `moveBlocksTo`, which a drop calls with the sibling index it lands
//! before. It adds no nodes or marks; the handle itself is host UI.
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_core::command;
//! use canvist_core::commands;
//! use canvist_extension_document::Document;
//! use canvist_extension_drag_handle::DragHandle;
//! use canvist_extension_paragraph::Paragraph;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let drag = builder.add(DragHandle);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! editor.insert_text("first", &mut env)?;
//! editor.run(&command(commands::split_block), &mut env)?;
//! editor.insert_text("second", &mut env)?;
//! editor.run(&drag.up(), &mut env)?;
//!
//! let doc = editor.state().doc();
//! let first = doc.first_text_block().and_then(|id| doc.inline(id));
//! assert_eq!(first.map(|text| text.as_str()), Some("second"));
//! # Ok::<(), canvist_core::EditorError>(())
//! ```
use std::ops::Range;

use canvist_core::Binding;
use canvist_core::Command;
use canvist_core::CommandResult;
use canvist_core::CommandSpec;
use canvist_core::EditKind;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::Field;
use canvist_core::Platform;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::ValueType;
use canvist_core::command;
use canvist_core::position;
use canvist_model::BlockId;
use canvist_model::Document;
use canvist_model::Schema;
use canvist_model::Selection;

/// The commands behind TipTap's `DragHandle`.
///
/// The handle itself is host UI: the host places it with the layout's
/// `block_rect` and finds where a drop lands with `hit_block`. These
/// commands do the move. `moveBlockUp` and `moveBlockDown` (Alt-↑ and
/// Alt-↓) swap the selected blocks with their neighbour, reaching for the
/// enclosing block when they are already at the edge of their container,
/// so a lone paragraph in a list item moves the item. `moveBlocksTo` puts
/// them before sibling `index`, as a drop indicator does.
#[derive(Clone, Copy, Debug, Default)]
pub struct DragHandle;

/// Typed access to [`DragHandle`].
#[derive(Clone, Copy, Debug)]
pub struct DragHandleHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl DragHandleHandle {
	/// Move the selected blocks up past their previous sibling.
	#[must_use]
	pub fn up(self) -> Command {
		up()
	}

	/// Move the selected blocks down past their next sibling.
	#[must_use]
	pub fn down(self) -> Command {
		down()
	}

	/// Move the selected blocks to just before sibling `index`, or to the
	/// end of their container when `index` is past the last.
	#[must_use]
	pub fn move_to(self, index: usize) -> Command {
		command(move |state| move_to(state, index))
	}
}

/// Sibling blocks: their parent and their index range in it.
type Siblings = (Option<BlockId>, Range<usize>);

fn up() -> Command {
	command(|state| step(state, false))
}

fn down() -> Command {
	command(|state| step(state, true))
}

/// The selected sibling blocks. A block selection, as a click on a drag
/// handle or a horizontal rule makes, moves the blocks it covers.
fn selected(state: &EditorState) -> Option<Siblings> {
	let doc = state.doc();

	let (first, last) = match state.selection() {
		Selection::Blocks { anchor, head }
			if position::compare_blocks(doc, anchor, head).is_gt() =>
		{
			(head, anchor)
		}
		Selection::Blocks { anchor, head } => (anchor, head),
		selection @ Selection::Text { .. } => {
			let range = position::text_range(doc, selection)?;
			(range.from.block, range.to.block)
		}
	};

	position::sibling_range(doc, first, last)
}

/// `range` of `parent`'s children, or the nearest enclosing block that can
/// move `down` (or up) when `range` is at its container's edge.
fn movable(doc: &Document, (mut parent, mut range): Siblings, down: bool) -> Option<Siblings> {
	loop {
		let free = if down {
			range.end < doc.children(parent).len()
		} else {
			range.start > 0
		};

		if free {
			return Some((parent, range));
		}

		let container = parent?;
		let index = doc.index_in_parent(container)?;
		parent = position::parent_of(doc, container);
		range = index..index + 1;
	}
}

fn step(state: &mut EditorState, down: bool) -> CommandResult {
	let Some((parent, range)) = selected(state).and_then(|found| movable(state.doc(), found, down))
	else {
		return Ok(None);
	};
	let to = if down {
		range.start + 1
	} else {
		range.start - 1
	};

	shift(state, parent, range, to)
}

fn move_to(state: &mut EditorState, index: usize) -> CommandResult {
	let Some((parent, range)) = selected(state) else {
		return Ok(None);
	};
	let index = index.min(state.doc().children(parent).len());

	if (range.start..=range.end).contains(&index) {
		return Ok(None);
	}

	// The target is counted after the moving blocks are taken out.
	let to = if index > range.end {
		index - range.len()
	} else {
		index
	};

	shift(state, parent, range, to)
}

/// Move `range` of `parent`'s children to `to`, counted after removal,
/// keeping the selection on them.
fn shift(
	state: &mut EditorState,
	parent: Option<BlockId>,
	range: Range<usize>,
	to: usize,
) -> CommandResult {
	let selection = state.selection();

	state.change(EditKind::Structure, |tr| {
		tr.move_blocks((parent, range.start, range.len()), (parent, to))?;
		tr.set_selection(selection);
		Ok(true)
	})
}

impl Extension for DragHandle {
	fn name(&self) -> &'static str {
		"dragHandle"
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let to = CommandSpec::new(
			"moveBlocksTo",
			"Move the selected blocks to just before a sibling, by its index, as a drop does.",
			|args| {
				let index = usize::try_from(args.int("index")?).map_err(|_| {
					EditorError::InvalidArgs {
						name: "moveBlocksTo".to_owned(),
						reason: "`index` must not be negative".to_owned(),
					}
				})?;
				Ok(command(move |state| move_to(state, index)))
			},
		)
		.arg(Field::required(
			"index",
			ValueType::Int,
			"The sibling to move before, from zero; past the last moves to the end.",
		));

		Ok(vec![
			CommandSpec::simple(
				"moveBlockUp",
				"Move the selected blocks up past their previous sibling.",
				up(),
			),
			CommandSpec::simple(
				"moveBlockDown",
				"Move the selected blocks down past their next sibling.",
				down(),
			),
			to,
		])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![("Alt-ArrowUp", up()), ("Alt-ArrowDown", down())])
	}
}

impl TypedExtension for DragHandle {
	type Handle = DragHandleHandle;

	fn handle(&self, registration: Registration) -> DragHandleHandle {
		DragHandleHandle {
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
	use canvist_extension_paragraph::Paragraph;

	use super::*;

	/// Paragraphs holding `texts`, with the caret in the last.
	fn paragraphs(texts: &[&str]) -> (Editor, DragHandleHandle) {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		let drag = builder.add(DragHandle);
		let mut editor = builder.build().expect("valid editor");
		let mut env = Env::headless(Duration::ZERO);

		for (index, text) in texts.iter().enumerate() {
			if index > 0 {
				editor
					.run(&command(commands::split_block), &mut env)
					.expect("split");
			}

			editor.insert_text(text, &mut env).expect("typed");
		}

		(editor, drag)
	}

	fn texts(editor: &Editor) -> Vec<String> {
		let doc = editor.state().doc();
		doc.text_blocks()
			.filter_map(|block| block.inline())
			.map(|text| text.as_str().to_owned())
			.collect()
	}

	fn run(editor: &mut Editor, command: &Command) -> bool {
		let mut env = Env::headless(Duration::ZERO);
		editor.run(command, &mut env).expect("ran").is_some()
	}

	#[test]
	fn up_and_down_swap_with_a_neighbour() {
		let (mut editor, drag) = paragraphs(&["a", "b", "c"]);

		assert!(run(&mut editor, &drag.up()));
		assert_eq!(texts(&editor), ["a", "c", "b"]);

		assert!(run(&mut editor, &drag.down()));
		assert_eq!(texts(&editor), ["a", "b", "c"]);
	}

	#[test]
	fn nothing_moves_past_the_edge_of_the_document() {
		let (mut editor, drag) = paragraphs(&["a", "b"]);

		assert!(!run(&mut editor, &drag.down()));
		assert_eq!(texts(&editor), ["a", "b"]);
	}

	#[test]
	fn move_to_counts_the_target_before_the_move() {
		let (mut editor, drag) = paragraphs(&["a", "b", "c"]);

		assert!(run(&mut editor, &drag.move_to(0)));
		assert_eq!(texts(&editor), ["c", "a", "b"]);

		assert!(run(&mut editor, &drag.move_to(99)));
		assert_eq!(texts(&editor), ["a", "b", "c"]);

		assert!(!run(&mut editor, &drag.move_to(2)));
		assert!(!run(&mut editor, &drag.move_to(3)));
	}
}
