#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_subscript`
//!
//! TipTap's `subscript` mark for the canvist editor, as in TipTap's
//! `@tiptap/extension-subscript`. [`Subscript`] registers the mark with the
//! `toggleSubscript`, `setSubscript`, and `unsetSubscript` commands and the
//! `isSubscript` query, and `Mod-,` toggles it. When `superscript` is
//! registered too, the two exclude each other. There are no input or paste
//! rules. Its handle is a [`MarkHandle`].
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//! use canvist_extension_subscript::Subscript;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let subscript = builder.add(Subscript);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! editor.run(&subscript.toggle(), &mut env)?;
//! assert!(subscript.is_active(editor.state()));
//! # Ok::<(), canvist_core::EditorError>(())
//! ```

use canvist_core::Binding;
use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::Extension;
use canvist_core::InputRule;
pub use canvist_core::MarkHandle;
use canvist_core::NodeMeta;
use canvist_core::PasteRule;
use canvist_core::Platform;
use canvist_core::QuerySpec;
use canvist_core::Registration;
use canvist_core::SimpleMark;
use canvist_core::TypedExtension;
use canvist_model::MarkSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;

static SUBSCRIPT: SimpleMark = SimpleMark {
	name: "subscript",
	toggle: "toggleSubscript",
	set: "setSubscript",
	unset: "unsetSubscript",
	is_active: "isSubscript",
	keys: &["Mod-,"],
	input: &[],
	paste: &[],
};

/// Subscript text: `Mod-,`. Subscript and superscript exclude each other.
#[derive(Clone, Copy, Debug, Default)]
pub struct Subscript;

impl Extension for Subscript {
	fn name(&self) -> &'static str {
		SUBSCRIPT.name
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark(MarkSpec::new("subscript").excludes_registered(&["superscript"]))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		SUBSCRIPT.nodes()
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(SUBSCRIPT.commands())
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(SUBSCRIPT.queries())
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(SUBSCRIPT.keymap())
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		SUBSCRIPT.input_rules(schema)
	}

	fn paste_rules(&self, schema: &Schema) -> Result<Vec<PasteRule>, EditorError> {
		SUBSCRIPT.paste_rules(schema)
	}
}

impl TypedExtension for Subscript {
	type Handle = MarkHandle;

	fn handle(&self, registration: Registration) -> MarkHandle {
		SUBSCRIPT.handle(registration)
	}
}

#[cfg(test)]
mod tests {
	use std::time::Duration;

	use canvist_core::Editor;
	use canvist_core::Env;
	use canvist_extension_document::Document;
	use canvist_extension_paragraph::Paragraph;
	use canvist_model::Attrs;
	use canvist_model::BlockSpec;
	use canvist_model::Point;
	use canvist_model::Selection;
	use canvist_model::Value;

	use super::*;

	#[test]
	fn toggle_adds_and_removes_the_mark() -> Result<(), EditorError> {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		let subscript = builder.add(Subscript);
		let mut editor = builder.build()?;
		let mut env = Env::headless(Duration::ZERO);
		editor.insert_text("ab", &mut env)?;
		let block = editor.state().doc().children(None)[0];
		editor.select(
			Selection::text(Point::new(block, 0), Point::new(block, 2)),
			&mut env,
		)?;

		editor.run(&subscript.toggle(), &mut env)?;
		assert!(subscript.is_active(editor.state()));
		assert_eq!(
			editor.query("isSubscript", &Attrs::new())?,
			Value::Bool(true)
		);

		editor.run(&subscript.toggle(), &mut env)?;
		assert!(!subscript.is_active(editor.state()));
		Ok(())
	}

	#[test]
	fn excludes_superscript_when_it_is_registered() {
		let paragraph = || Schema::builder().block(BlockSpec::text("paragraph"));
		let alone = Subscript.schema(paragraph()).build().expect("valid schema");
		let both = Subscript
			.schema(paragraph().mark(MarkSpec::new("superscript")))
			.build()
			.expect("valid schema");
		let subscript = both
			.mark_type("subscript")
			.expect("subscript is registered");
		let superscript = both
			.mark_type("superscript")
			.expect("superscript is registered");

		assert!(alone.mark_type("subscript").is_some());
		assert!(both.mark(subscript).excludes.contains(&superscript));
	}
}
