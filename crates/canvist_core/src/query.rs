//! Reading the selection's surroundings for an extension's queries.
//!
//! Extensions answer questions such as "which heading level is the caret
//! in?" or "which colour does the selection share?" by name, since their
//! handles exist before the schema does. Each helper returns `None` (or
//! `false`) when the schema lacks the type, so a query run against an
//! editor without the extension reports nothing instead of failing.

use std::ops::Range;

use canvist_model::BlockId;
use canvist_model::MarkTypeId;
use canvist_model::Selection;
use canvist_model::Value;

use crate::EditorState;
use crate::commands;
use crate::position;

/// The block holding the selection head (or the first selected block).
fn head_block(state: &EditorState) -> BlockId {
	match state.selection() {
		Selection::Text { head, .. } => head.block,
		Selection::Blocks { anchor, .. } => anchor,
	}
}

/// The nearest block of type `name` around the selection head, the head's
/// own block included.
///
/// ```
/// use std::sync::Arc;
///
/// use canvist_core::EditorState;
/// use canvist_core::query;
/// use canvist_model::BlockSpec;
/// use canvist_model::Document;
/// use canvist_model::Schema;
///
/// let schema = Arc::new(Schema::builder().block(BlockSpec::text("paragraph")).build()?);
/// let state = EditorState::new(Document::new(schema, 1))?;
///
/// assert!(query::around(&state, "paragraph").is_some());
/// assert!(query::around(&state, "heading").is_none());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn around(state: &EditorState, name: &str) -> Option<BlockId> {
	let kind = state.schema().block_type(name)?;
	let doc = state.doc();

	position::ancestors_or_self(doc, head_block(state))
		.find(|&id| doc.block(id).is_some_and(|block| block.kind() == kind))
}

/// Attribute `key` of the nearest block of type `name` around the head.
pub fn attr_around(state: &EditorState, name: &str, key: &str) -> Option<Value> {
	let id = around(state, name)?;
	state.doc().block(id)?.attrs().get(key).cloned()
}

/// Whether mark `name` is active at the selection: on every selected
/// character, or among the caret's active marks.
pub fn mark_active(state: &EditorState, name: &str) -> bool {
	state
		.schema()
		.mark_type(name)
		.is_some_and(|kind| commands::is_mark_active(state, kind))
}

/// Attribute `key` of mark `name` where every selected character (or the
/// caret's active marks) carries the mark with that one value, so a query
/// never reports the first run's colour for a mixed selection.
pub fn shared_mark_attr(state: &EditorState, name: &str, key: &str) -> Option<Value> {
	let kind = state.schema().mark_type(name)?;

	if !commands::is_mark_active(state, kind) {
		return None;
	}

	let ranges = position::selected_text(state.doc(), state.selection());
	let mut values = ranges
		.iter()
		.filter(|(_, range)| !range.is_empty())
		.flat_map(|(block, range)| run_attrs(state, *block, range.clone(), kind, key));

	let Some(first) = values.next() else {
		let marks = commands::active_marks(state);
		return marks.get(kind)?.attrs.get(key).cloned();
	};

	values.all(|value| value == first).then_some(first)
}

/// The value of `key` on mark `kind` in each run of `range` of `block`.
fn run_attrs(
	state: &EditorState,
	block: BlockId,
	range: Range<u32>,
	kind: MarkTypeId,
	key: &str,
) -> Vec<Value> {
	let Some(runs) = state
		.doc()
		.inline(block)
		.and_then(|text| text.mark_runs(range).ok())
	else {
		return Vec::new();
	};

	runs.iter()
		.map(|(_, marks)| {
			marks
				.get(kind)
				.and_then(|mark| mark.attrs.get(key).cloned())
				.unwrap_or_default()
		})
		.collect()
}
