#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_superscript`
//!
//! TipTap's `superscript` mark for the canvist editor. [`Superscript`] registers the
//! mark with `toggleSuperscript`, `setSuperscript`, and `unsetSuperscript`, the `isSuperscript` query, and its keys
//! and Markdown shortcuts. Its handle is a [`MarkHandle`].
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//! use canvist_extension_superscript::Superscript;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let superscript = builder.add(Superscript);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! editor.run(&superscript.toggle(), &mut env)?;
//! assert!(superscript.is_active(editor.state()));
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

static SUPERSCRIPT: SimpleMark = SimpleMark {
	name: "superscript",
	toggle: "toggleSuperscript",
	set: "setSuperscript",
	unset: "unsetSuperscript",
	is_active: "isSuperscript",
	keys: &["Mod-."],
	input: &[],
	paste: &[],
};

/// Superscript text: `Mod-.`. Subscript and superscript exclude each other.
#[derive(Clone, Copy, Debug, Default)]
pub struct Superscript;

impl Extension for Superscript {
	fn name(&self) -> &'static str {
		SUPERSCRIPT.name
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark(MarkSpec::new("superscript").excludes_registered(&["subscript"]))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		SUPERSCRIPT.nodes()
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(SUPERSCRIPT.commands())
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(SUPERSCRIPT.queries())
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(SUPERSCRIPT.keymap())
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		SUPERSCRIPT.input_rules(schema)
	}

	fn paste_rules(&self, schema: &Schema) -> Result<Vec<PasteRule>, EditorError> {
		SUPERSCRIPT.paste_rules(schema)
	}
}

impl TypedExtension for Superscript {
	type Handle = MarkHandle;

	fn handle(&self, registration: Registration) -> MarkHandle {
		SUPERSCRIPT.handle(registration)
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
			.mark(MarkSpec::new("superscript"))
			.build()
			.expect("valid schema");

		assert!(SUPERSCRIPT.input_rules(&schema).is_ok());
		assert!(SUPERSCRIPT.paste_rules(&schema).is_ok());
	}
}
