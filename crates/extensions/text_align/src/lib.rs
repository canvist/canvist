#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_text_align`
//!
//! TipTap's text alignment for the canvist editor, as in TipTap's
//! `@tiptap/extension-text-align`. [`TextAlign`] adds a nullable
//! `textAlign` attribute (`left`, `center`, `right`, or `justify`) to the
//! block types it names, headings and paragraphs by default. It registers
//! the `setTextAlign`, `unsetTextAlign`, and `toggleTextAlign` commands and
//! the `textAlign` query, and binds `Mod-Shift-l`, `e`, `r`, and `j` to
//! left, centre, right, and justified.
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//! use canvist_extension_text_align::Alignment;
//! use canvist_extension_text_align::TextAlign;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let align = builder.add(TextAlign::default());
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! editor.run(&align.set(Alignment::Center), &mut env)?;
//! assert_eq!(align.alignment(editor.state()), Some(Alignment::Center));
//!
//! editor.run(&align.toggle(Alignment::Center), &mut env)?;
//! assert_eq!(align.alignment(editor.state()), None);
//! # Ok::<(), canvist_core::EditorError>(())
//! ```

use canvist_core::Args;
use canvist_core::Binding;
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
use canvist_core::Platform;
use canvist_core::QuerySpec;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::ValueType;
use canvist_core::command;
use canvist_core::commands;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Value;

/// The attribute every aligned block carries.
const ATTR: &str = "textAlign";

/// The keys TipTap binds, one per alignment.
const KEYS: [(&str, Alignment); 4] = [
	("Mod-Shift-l", Alignment::Left),
	("Mod-Shift-e", Alignment::Center),
	("Mod-Shift-r", Alignment::Right),
	("Mod-Shift-j", Alignment::Justify),
];

/// A `textAlign` attribute on the text blocks named in `types`: left,
/// centre, right, or justified. Layout aligns the lines and server-rendered
/// HTML writes `text-align`. `Mod-Shift-l`, `e`, `r`, and `j` set it.
///
/// An unset alignment follows the text: left, or right for right-to-left
/// paragraphs.
#[derive(Clone, Debug)]
pub struct TextAlign {
	/// The block types that can be aligned. Types the schema lacks are
	/// skipped.
	pub types: Vec<&'static str>,
}

impl Default for TextAlign {
	fn default() -> Self {
		Self {
			types: vec!["heading", "paragraph"],
		}
	}
}

/// Typed access to [`TextAlign`].
#[derive(Clone, Copy, Debug)]
pub struct TextAlignHandle {
	_registration: Registration,
}

/// An alignment for [`TextAlignHandle`]: the ones TipTap offers, which are
/// also CSS `text-align` values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Alignment {
	/// The left edge.
	Left,
	/// Centred.
	Center,
	/// The right edge.
	Right,
	/// Both edges.
	Justify,
}

impl Alignment {
	const ALL: [Self; 4] = [Self::Left, Self::Center, Self::Right, Self::Justify];

	/// The attribute value, such as `center`.
	#[must_use]
	pub const fn name(self) -> &'static str {
		match self {
			Self::Left => "left",
			Self::Center => "center",
			Self::Right => "right",
			Self::Justify => "justify",
		}
	}

	fn from_name(name: &str) -> Option<Self> {
		Self::ALL
			.into_iter()
			.find(|alignment| alignment.name() == name)
	}

	/// Every alignment's attribute value, as a metadata type.
	fn value_type() -> ValueType {
		ValueType::OneOf(Self::ALL.map(Self::name).to_vec())
	}
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl TextAlignHandle {
	/// Align the selected text blocks.
	#[must_use]
	pub fn set(self, alignment: Alignment) -> Command {
		set(Some(alignment))
	}

	/// Clear the alignment of the selected text blocks.
	#[must_use]
	pub fn unset(self) -> Command {
		set(None)
	}

	/// Clear the alignment if every selected text block has it, otherwise
	/// set it.
	#[must_use]
	pub fn toggle(self, alignment: Alignment) -> Command {
		toggle(alignment)
	}

	/// The alignment every selected text block shares, if they share one.
	#[must_use]
	pub fn alignment(self, state: &EditorState) -> Option<Alignment> {
		shared_alignment(state)
	}
}

/// The selected text blocks that have the `textAlign` attribute.
fn alignable(state: &EditorState) -> Vec<BlockId> {
	let doc = state.doc();
	let schema = state.schema();

	commands::selected_text_blocks(state)
		.into_iter()
		.filter(|&id| {
			doc.block(id)
				.is_some_and(|block| schema.block(block.kind()).attrs.get(ATTR).is_some())
		})
		.collect()
}

/// The alignment of `block`, or `None` when it has none.
fn alignment_of(state: &EditorState, block: BlockId) -> Option<String> {
	let value = state.doc().block(block)?.attrs().get(ATTR)?;
	value.as_str().map(str::to_owned)
}

/// The alignment every alignable selected block shares.
fn shared(state: &EditorState) -> Option<String> {
	let blocks = alignable(state);
	let first = alignment_of(state, *blocks.first()?)?;

	blocks
		.iter()
		.all(|&block| alignment_of(state, block).as_deref() == Some(&first))
		.then_some(first)
}

/// [`shared`], as an [`Alignment`].
fn shared_alignment(state: &EditorState) -> Option<Alignment> {
	shared(state).as_deref().and_then(Alignment::from_name)
}

/// Set every alignable selected block's alignment, or clear it for `None`.
fn set_alignment(state: &mut EditorState, alignment: Option<Alignment>) -> CommandResult {
	let value = alignment.map_or(Value::Null, |alignment| Value::from(alignment.name()));
	let blocks: Vec<_> = alignable(state)
		.into_iter()
		.filter_map(|id| {
			let block = state.doc().block(id)?;
			let attrs = block.attrs();
			let changed = *attrs.get(ATTR).unwrap_or(&Value::Null) != value;
			changed.then(|| (id, block.kind(), aligned(attrs, &value)))
		})
		.collect();

	if blocks.is_empty() {
		return Ok(None);
	}

	state.change(EditKind::Structure, |tr| {
		for (id, kind, attrs) in blocks {
			tr.set_block(id, kind, attrs)?;
		}

		Ok(true)
	})
}

/// `attrs` with the alignment `value`. Clearing removes the attribute, as a
/// block that was never aligned does not have it.
fn aligned(attrs: &Attrs, value: &Value) -> Attrs {
	let mut attrs = attrs.clone();

	if *value == Value::Null {
		attrs.remove(ATTR);
	} else {
		attrs.set(ATTR, value.clone());
	}

	attrs
}

fn set(alignment: Option<Alignment>) -> Command {
	command(move |state| set_alignment(state, alignment))
}

fn toggle(alignment: Alignment) -> Command {
	command(move |state| {
		let next = (shared_alignment(state) != Some(alignment)).then_some(alignment);
		set_alignment(state, next)
	})
}

fn alignment_field() -> Field {
	Field::required(
		"alignment",
		Alignment::value_type(),
		"The alignment: left, center, right, or justify.",
	)
}

fn alignment_arg(args: &Args<'_>) -> Result<Alignment, EditorError> {
	let name = args.string("alignment")?;
	Alignment::from_name(name).ok_or_else(|| {
		EditorError::InvalidArgs {
			name: "textAlign".to_owned(),
			reason: format!("no alignment `{name}`"),
		}
	})
}

impl Extension for TextAlign {
	fn name(&self) -> &'static str {
		ATTR
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.block_attrs(&self.types, Attrs::new().with(ATTR, Value::Null))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		let field = Field::optional(
			ATTR,
			ValueType::nullable(Alignment::value_type()),
			Value::Null,
			"How the block's lines align, or null to follow the text direction.",
		);

		self.types
			.iter()
			.map(|&name| NodeMeta::new(NodeKind::Block, name).attr(field.clone()))
			.collect()
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let set_spec =
			CommandSpec::new("setTextAlign", "Align the selected text blocks.", |args| {
				Ok(set(Some(alignment_arg(args)?)))
			});
		let toggle_spec = CommandSpec::new(
			"toggleTextAlign",
			"Clear the alignment if every selected text block has it, otherwise set it.",
			|args| Ok(toggle(alignment_arg(args)?)),
		);

		Ok(vec![
			set_spec.arg(alignment_field()),
			CommandSpec::simple(
				"unsetTextAlign",
				"Clear the alignment of the selected text blocks.",
				set(None),
			),
			toggle_spec.arg(alignment_field()),
		])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(vec![QuerySpec::new(
			ATTR,
			"The alignment every selected text block shares, or null.",
			ValueType::nullable(Alignment::value_type()),
			|state, _| Ok(shared(state).map_or(Value::Null, Value::from)),
		)])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(KEYS
			.into_iter()
			.map(|(key, alignment)| (key, set(Some(alignment))))
			.collect())
	}
}

impl TypedExtension for TextAlign {
	type Handle = TextAlignHandle;

	fn handle(&self, registration: Registration) -> TextAlignHandle {
		TextAlignHandle {
			_registration: registration,
		}
	}
}

#[cfg(test)]
mod tests {
	use std::time::Duration;

	use canvist_core::Editor;
	use canvist_core::Env;
	use canvist_core::KeyPress;
	use canvist_core::Modifiers;
	use canvist_extension_document::Document;
	use canvist_extension_paragraph::Paragraph;
	use canvist_model::Point;
	use canvist_model::Selection;

	use super::*;

	fn editor(align: TextAlign) -> Result<(Editor, TextAlignHandle), EditorError> {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		let handle = builder.add(align);
		Ok((builder.build()?, handle))
	}

	/// The first block's `textAlign` attribute, if it has one.
	fn first_attr(editor: &Editor) -> Option<Value> {
		let doc = editor.state().doc();
		doc.block(doc.children(None)[0])?.attrs().get(ATTR).cloned()
	}

	#[test]
	fn set_toggle_and_unset() -> Result<(), EditorError> {
		let (mut editor, align) = editor(TextAlign::default())?;
		let mut env = Env::headless(Duration::ZERO);

		editor.run(&align.set(Alignment::Right), &mut env)?;
		assert_eq!(first_attr(&editor), Some(Value::from("right")));
		editor.run(&align.toggle(Alignment::Justify), &mut env)?;
		assert_eq!(align.alignment(editor.state()), Some(Alignment::Justify));
		editor.run(&align.toggle(Alignment::Justify), &mut env)?;
		assert_eq!(first_attr(&editor), None);

		editor.run(&align.set(Alignment::Left), &mut env)?;
		editor.run(&align.unset(), &mut env)?;
		assert_eq!(first_attr(&editor), None);
		assert_eq!(editor.run(&align.unset(), &mut env)?, None);
		Ok(())
	}

	#[test]
	fn keys_and_named_commands_align() -> Result<(), EditorError> {
		let (mut editor, align) = editor(TextAlign::default())?;
		let mut env = Env::headless(Duration::ZERO);
		let mod_shift = Modifiers {
			ctrl: true,
			shift: true,
			..Modifiers::NONE
		};

		editor.handle_key(KeyPress::new("e", mod_shift), &mut env)?;
		assert_eq!(align.alignment(editor.state()), Some(Alignment::Center));

		let right = Attrs::new().with("alignment", "right");
		editor.run_named("setTextAlign", &right, &mut env)?;
		assert_eq!(editor.query(ATTR, &Attrs::new())?, Value::from("right"));

		let middle = Attrs::new().with("alignment", "middle");
		assert!(editor.run_named("setTextAlign", &middle, &mut env).is_err());
		Ok(())
	}

	#[test]
	fn mixed_alignments_share_none() -> Result<(), EditorError> {
		let (mut editor, align) = editor(TextAlign::default())?;
		let mut env = Env::headless(Duration::ZERO);
		editor.insert_text("a", &mut env)?;
		editor.run(&align.set(Alignment::Left), &mut env)?;
		editor.run(&command(commands::split_block), &mut env)?;
		editor.run(&align.set(Alignment::Right), &mut env)?;

		let blocks = editor.state().doc().children(None).to_vec();
		let both = Selection::text(Point::new(blocks[0], 0), Point::new(blocks[1], 0));
		editor.select(both, &mut env)?;
		assert_eq!(align.alignment(editor.state()), None);

		editor.run(&align.toggle(Alignment::Right), &mut env)?;
		assert_eq!(align.alignment(editor.state()), Some(Alignment::Right));
		Ok(())
	}

	#[test]
	fn unlisted_types_are_left_alone() -> Result<(), EditorError> {
		let headings_only = TextAlign {
			types: vec!["heading"],
		};
		let (mut editor, align) = editor(headings_only)?;
		let mut env = Env::headless(Duration::ZERO);

		assert_eq!(editor.run(&align.set(Alignment::Center), &mut env)?, None);
		assert_eq!(first_attr(&editor), None);
		assert_eq!(align.alignment(editor.state()), None);
		Ok(())
	}
}
