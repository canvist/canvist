#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_underline`
//!
//! TipTap's `underline` mark for the canvist editor. [`Underline`] registers the
//! mark with `toggleUnderline`, `setUnderline`, and `unsetUnderline`, the `isUnderline` query, and its keys
//! and Markdown shortcuts. Its handle is a [`MarkHandle`].
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
	use canvist_model::BlockSpec;

	use super::*;

	#[test]
	fn every_pattern_compiles() {
		let schema = Schema::builder()
			.block(BlockSpec::text("paragraph"))
			.mark(MarkSpec::new("underline"))
			.build()
			.expect("valid schema");

		assert!(UNDERLINE.input_rules(&schema).is_ok());
		assert!(UNDERLINE.paste_rules(&schema).is_ok());
	}
}
