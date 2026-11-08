#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_paragraph`
//!
//! TipTap's `paragraph` node for the canvist editor, as in TipTap's
//! `@tiptap/extension-paragraph`. [`Paragraph`] registers the default text
//! block, which Enter creates and an empty document starts with, along with
//! the `setParagraph` command, the `isParagraph` query, and `Mod-Alt-0`,
//! which turns the selected text blocks back into paragraphs.
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//!
//! let mut builder = Editor::builder().extension(Document);
//! let paragraph = builder.add(Paragraph);
//! let mut editor = builder.build()?;
//!
//! let mut env = Env::headless(Duration::ZERO);
//!
//! editor.insert_text("Hello", &mut env)?;
//! assert!(paragraph.is_active(editor.state()));
//! # Ok::<(), canvist_core::EditorError>(())
//! ```

use canvist_core::Binding;
use canvist_core::Command;
use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::Platform;
use canvist_core::QuerySpec;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::ValueType;
use canvist_core::block_type;
use canvist_core::command;
use canvist_core::commands;
use canvist_model::Attrs;
use canvist_model::BlockSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Value;

/// The default text block. It registers first (priority 1000), so it is
/// what Enter creates and what an empty document starts with.
#[derive(Clone, Copy, Debug, Default)]
pub struct Paragraph;

/// Typed access to [`Paragraph`].
#[derive(Clone, Copy, Debug)]
pub struct ParagraphHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl ParagraphHandle {
	/// Turn the selected text blocks into paragraphs.
	#[must_use]
	pub fn set(self) -> Command {
		set()
	}

	/// Whether every selected text block is a paragraph.
	#[must_use]
	pub fn is_active(self, state: &EditorState) -> bool {
		is_active(state)
	}
}

fn set() -> Command {
	command(|state| {
		let paragraph = block_type(state.schema(), "paragraph", "paragraph")?;
		commands::set_block_type(state, paragraph, &Attrs::new())
	})
}

fn is_active(state: &EditorState) -> bool {
	let paragraph = state.schema().block_type("paragraph");
	paragraph.is_some_and(|kind| commands::is_block_active(state, kind, &Attrs::new()))
}

impl Extension for Paragraph {
	fn name(&self) -> &'static str {
		"paragraph"
	}

	fn priority(&self) -> i32 {
		1000
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.block(BlockSpec::text("paragraph"))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Block, "paragraph")]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(vec![CommandSpec::simple(
			"setParagraph",
			"Turn the selected text blocks into paragraphs.",
			set(),
		)])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		let query = QuerySpec::new(
			"isParagraph",
			"Whether every selected text block is a paragraph.",
			ValueType::Bool,
			|state, _| Ok(Value::Bool(is_active(state))),
		);
		Ok(vec![query])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![("Mod-Alt-0", set())])
	}
}

impl TypedExtension for Paragraph {
	type Handle = ParagraphHandle;

	fn handle(&self, registration: Registration) -> ParagraphHandle {
		ParagraphHandle {
			_registration: registration,
		}
	}
}

#[cfg(test)]
mod tests {
	use std::time::Duration;

	use canvist_core::Editor;
	use canvist_core::Env;
	use canvist_extension_document::Document;

	use super::*;

	/// Another text block, registered before the paragraph.
	struct Title;

	impl Extension for Title {
		fn name(&self) -> &'static str {
			"title"
		}

		fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
			schema.block(BlockSpec::text("title"))
		}
	}

	#[test]
	fn is_the_default_text_block_whatever_the_order() -> Result<(), EditorError> {
		let mut builder = Editor::builder().extension(Document).extension(Title);
		let paragraph = builder.add(Paragraph);
		let editor = builder.build()?;

		assert!(paragraph.is_active(editor.state()));
		assert_eq!(
			editor.schema().default_text_block(),
			block_type(editor.schema(), "paragraph", "paragraph")?
		);
		Ok(())
	}

	#[test]
	fn set_turns_other_text_blocks_into_paragraphs() -> Result<(), EditorError> {
		let mut builder = Editor::builder().extension(Document).extension(Title);
		let paragraph = builder.add(Paragraph);
		let mut editor = builder.build()?;
		let mut env = Env::headless(Duration::ZERO);
		let title = block_type(editor.schema(), "title", "title")?;
		let to_title = command(move |state| commands::set_block_type(state, title, &Attrs::new()));

		editor.run(&to_title, &mut env)?;
		assert!(!paragraph.is_active(editor.state()));
		assert_eq!(
			editor.query("isParagraph", &Attrs::new())?,
			Value::Bool(false)
		);

		editor.run(&paragraph.set(), &mut env)?;
		assert!(paragraph.is_active(editor.state()));
		Ok(())
	}
}
