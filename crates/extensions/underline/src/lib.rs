#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_underline`
//!
//! TipTap's `underline` mark for the canvist editor, as in TipTap's
//! `@tiptap/extension-underline`. [`Underline`] registers the mark with the
//! `toggleUnderline`, `setUnderline`, and `unsetUnderline` commands and the
//! `isUnderline` query, and `Mod-u` toggles it. Markdown has no underline,
//! so there are no input or paste rules. Its handle is a [`MarkHandle`].
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//! use canvist_extension_underline::Underline;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let underline = builder.add(Underline);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! editor.run(&underline.toggle(), &mut env)?;
//! assert!(underline.is_active(editor.state()));
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

static UNDERLINE: SimpleMark = SimpleMark {
	name: "underline",
	toggle: "toggleUnderline",
	set: "setUnderline",
	unset: "unsetUnderline",
	is_active: "isUnderline",
	keys: &["Mod-u"],
	input: &[],
	paste: &[],
};

/// Underlined text: `Mod-u`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Underline;

impl Extension for Underline {
	fn name(&self) -> &'static str {
		UNDERLINE.name
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark(MarkSpec::new("underline"))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		UNDERLINE.nodes()
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(UNDERLINE.commands())
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(UNDERLINE.queries())
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(UNDERLINE.keymap())
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		UNDERLINE.input_rules(schema)
	}

	fn paste_rules(&self, schema: &Schema) -> Result<Vec<PasteRule>, EditorError> {
		UNDERLINE.paste_rules(schema)
	}
}

impl TypedExtension for Underline {
	type Handle = MarkHandle;

	fn handle(&self, registration: Registration) -> MarkHandle {
		UNDERLINE.handle(registration)
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
	use canvist_model::Point;
	use canvist_model::Selection;
	use canvist_model::Value;

	use super::*;

	#[test]
	fn toggle_adds_and_removes_the_mark() -> Result<(), EditorError> {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		let underline = builder.add(Underline);
		let mut editor = builder.build()?;
		let mut env = Env::headless(Duration::ZERO);
		editor.insert_text("ab", &mut env)?;
		let block = editor.state().doc().children(None)[0];
		editor.select(
			Selection::text(Point::new(block, 0), Point::new(block, 2)),
			&mut env,
		)?;

		editor.run(&underline.toggle(), &mut env)?;
		assert!(underline.is_active(editor.state()));
		assert_eq!(
			editor.query("isUnderline", &Attrs::new())?,
			Value::Bool(true)
		);

		editor.run(&underline.toggle(), &mut env)?;
		assert!(!underline.is_active(editor.state()));
		Ok(())
	}
}
