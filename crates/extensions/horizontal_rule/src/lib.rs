#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_horizontal_rule`
//!
//! TipTap's `horizontalRule` node for the canvist editor, as in TipTap's
//! `@tiptap/extension-horizontal-rule`. [`HorizontalRule`] registers an
//! atomic block, like `<hr>`, and the `setHorizontalRule` command. Typing
//! `---`, `___ `, or `*** ` on an empty line replaces it with a rule; so
//! does `—-`, which is what `---` becomes when typography turns `--` into
//! an em dash first.
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_horizontal_rule::HorizontalRule;
//! use canvist_extension_paragraph::Paragraph;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! builder.add(HorizontalRule);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! for character in "---".chars() {
//! 	editor.insert_text(&character.to_string(), &mut env)?;
//! }
//!
//! let doc = editor.state().doc();
//! let first = doc.block(doc.children(None)[0]).map(|block| block.kind());
//! assert_eq!(first, editor.schema().block_type("horizontalRule"));
//! # Ok::<(), canvist_core::EditorError>(())
//! ```

use canvist_core::Command;
use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::Extension;
use canvist_core::InputRule;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::block_type;
use canvist_core::command;
use canvist_core::commands;
use canvist_model::Attrs;
use canvist_model::BlockSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;

/// A `horizontalRule` atomic block. `---`, `___ `, or `*** ` on an empty
/// line inserts one.
#[derive(Clone, Copy, Debug, Default)]
pub struct HorizontalRule;

/// Typed access to [`HorizontalRule`].
#[derive(Clone, Copy, Debug)]
pub struct HorizontalRuleHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl HorizontalRuleHandle {
	/// Insert a horizontal rule at the selection.
	#[must_use]
	pub fn insert(self) -> Command {
		insert()
	}
}

fn insert() -> Command {
	command(|state| {
		let kind = block_type(state.schema(), "horizontalRule", "horizontalRule")?;
		commands::insert_leaf(state, kind, Attrs::new())
	})
}

impl Extension for HorizontalRule {
	fn name(&self) -> &'static str {
		"horizontalRule"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.block(BlockSpec::leaf("horizontalRule"))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Block, "horizontalRule")]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(vec![CommandSpec::simple(
			"setHorizontalRule",
			"Insert a horizontal rule at the selection.",
			insert(),
		)])
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let kind = block_type(schema, self.name(), "horizontalRule")?;
		Ok(vec![InputRule::leaf(r"^(?:---|—-|___\s|\*\*\*\s)$", kind)?])
	}
}

impl TypedExtension for HorizontalRule {
	type Handle = HorizontalRuleHandle;

	fn handle(&self, registration: Registration) -> HorizontalRuleHandle {
		HorizontalRuleHandle {
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
	use canvist_model::Block;
	use canvist_model::BlockTypeId;

	use super::*;

	/// The top-level block types after typing `text` into an empty editor.
	fn typed(text: &str) -> Result<(Vec<BlockTypeId>, BlockTypeId), EditorError> {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		builder.add(HorizontalRule);
		let mut editor = builder.build()?;
		let mut env = Env::headless(Duration::ZERO);

		for character in text.chars() {
			editor.insert_text(&character.to_string(), &mut env)?;
		}

		let doc = editor.state().doc();
		let kinds = doc
			.children(None)
			.iter()
			.filter_map(|&id| doc.block(id).map(Block::kind))
			.collect();
		Ok((
			kinds,
			block_type(editor.schema(), "test", "horizontalRule")?,
		))
	}

	#[test]
	fn every_shortcut_inserts_a_rule() -> Result<(), EditorError> {
		for text in ["---", "___ ", "*** ", "—-"] {
			let (kinds, rule) = typed(text)?;
			assert_eq!(kinds.first(), Some(&rule), "{text}");
		}

		Ok(())
	}

	#[test]
	fn shortcuts_need_an_empty_line() -> Result<(), EditorError> {
		for text in ["a---", "___", "** *"] {
			let (kinds, rule) = typed(text)?;
			assert!(!kinds.contains(&rule), "{text}");
		}

		Ok(())
	}

	#[test]
	fn insert_adds_a_rule() -> Result<(), EditorError> {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		let rule = builder.add(HorizontalRule);
		let mut editor = builder.build()?;
		let mut env = Env::headless(Duration::ZERO);

		editor.run(&rule.insert(), &mut env)?;

		let kind = block_type(editor.schema(), "test", "horizontalRule")?;
		let doc = editor.state().doc();
		let has_rule = doc
			.children(None)
			.iter()
			.any(|&id| doc.block(id).is_some_and(|block| block.kind() == kind));
		assert!(has_rule);
		Ok(())
	}
}
