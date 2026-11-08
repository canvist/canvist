#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_blockquote`
//!
//! TipTap's `blockquote` node for the canvist editor, as in TipTap's
//! `@tiptap/extension-blockquote`. [`Blockquote`] registers a container
//! that holds any blocks, the `toggleBlockquote` command, and the
//! `isBlockquote` query. `Mod-Shift-b` wraps the selected blocks in a quote
//! or lifts them out of one, and typing `> ` at the start of a block wraps
//! it.
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_blockquote::Blockquote;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let quote = builder.add(Blockquote);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! for character in "> Quoted".chars() {
//! 	editor.insert_text(&character.to_string(), &mut env)?;
//! }
//!
//! assert!(quote.is_active(editor.state()));
//! assert_eq!(editor.state().doc().plain_text(), "Quoted");
//! # Ok::<(), canvist_core::EditorError>(())
//! ```

use canvist_core::Binding;
use canvist_core::Command;
use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::InputRule;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::Platform;
use canvist_core::QuerySpec;
use canvist_core::Registration;
use canvist_core::RuleMatch;
use canvist_core::TypedExtension;
use canvist_core::ValueType;
use canvist_core::block_type;
use canvist_core::command;
use canvist_core::commands;
use canvist_core::query;
use canvist_model::Attrs;
use canvist_model::BlockSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Value;

/// A `blockquote` container for any blocks. `> ` at the start of a block
/// wraps it, and `Mod-Shift-b` toggles one.
#[derive(Clone, Copy, Debug, Default)]
pub struct Blockquote;

/// Typed access to [`Blockquote`].
#[derive(Clone, Copy, Debug)]
pub struct BlockquoteHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl BlockquoteHandle {
	/// Wrap the selected blocks in a quote, or lift them out of one.
	#[must_use]
	pub fn toggle(self) -> Command {
		toggle()
	}

	/// Whether the selection is inside a quote.
	#[must_use]
	pub fn is_active(self, state: &EditorState) -> bool {
		query::around(state, "blockquote").is_some()
	}
}

fn toggle() -> Command {
	command(|state| {
		let quote = block_type(state.schema(), "blockquote", "blockquote")?;
		commands::toggle_wrap(state, quote, Attrs::new())
	})
}

impl Extension for Blockquote {
	fn name(&self) -> &'static str {
		"blockquote"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.block(BlockSpec::container("blockquote", &["block"]))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Block, "blockquote")]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let toggle_spec = CommandSpec::simple(
			"toggleBlockquote",
			"Wrap the selected blocks in a quote, or lift them out of one.",
			toggle(),
		);
		Ok(vec![toggle_spec])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		let query = QuerySpec::new(
			"isBlockquote",
			"Whether the selection is inside a quote.",
			ValueType::Bool,
			|state, _| Ok(Value::Bool(query::around(state, "blockquote").is_some())),
		);
		Ok(vec![query])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![("Mod-Shift-b", toggle())])
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let quote = block_type(schema, self.name(), "blockquote")?;
		let wrap = move |state: &mut EditorState, _: &RuleMatch| {
			commands::wrap_in(state, quote, Attrs::new())
		};
		Ok(vec![InputRule::wrap(r"^\s*>\s$", wrap)?])
	}
}

impl TypedExtension for Blockquote {
	type Handle = BlockquoteHandle;

	fn handle(&self, registration: Registration) -> BlockquoteHandle {
		BlockquoteHandle {
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
	use canvist_extension_paragraph::Paragraph;

	use super::*;

	fn editor() -> Result<(Editor, BlockquoteHandle), EditorError> {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		let quote = builder.add(Blockquote);
		Ok((builder.build()?, quote))
	}

	#[test]
	fn toggle_wraps_and_lifts() -> Result<(), EditorError> {
		let (mut editor, quote) = editor()?;
		let mut env = Env::headless(Duration::ZERO);
		editor.insert_text("Quoted", &mut env)?;

		editor.run(&quote.toggle(), &mut env)?;
		assert!(quote.is_active(editor.state()));
		assert_eq!(
			editor.query("isBlockquote", &Attrs::new())?,
			Value::Bool(true)
		);

		editor.run(&quote.toggle(), &mut env)?;
		assert!(!quote.is_active(editor.state()));
		assert_eq!(editor.state().doc().plain_text(), "Quoted");
		Ok(())
	}

	#[test]
	fn the_shortcut_only_fires_at_the_start_of_a_block() -> Result<(), EditorError> {
		let (mut editor, quote) = editor()?;
		let mut env = Env::headless(Duration::ZERO);

		for character in "a > b".chars() {
			editor.insert_text(&character.to_string(), &mut env)?;
		}

		assert!(!quote.is_active(editor.state()));
		assert_eq!(editor.state().doc().plain_text(), "a > b");
		Ok(())
	}
}
