#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_code`
//!
//! TipTap's `code` mark for the canvist editor. [`Code`] registers the
//! mark with `toggleCode`, `setCode`, and `unsetCode`, the `isCode` query, and its keys
//! and Markdown shortcuts. Its handle is a [`MarkHandle`].
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//! use canvist_extension_code::Code;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let code = builder.add(Code::default());
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! for character in "`code`".chars() {
//! 	editor.insert_text(&character.to_string(), &mut env)?;
//! }
//!
//! assert_eq!(editor.state().doc().plain_text(), "code");
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

static CODE: SimpleMark = SimpleMark {
	name: "code",
	toggle: "toggleCode",
	set: "setCode",
	unset: "unsetCode",
	is_active: "isCode",
	keys: &["Mod-e"],
	input: &[r"(?:^|\s)`([^`\s](?:[^`]*[^`\s])?)`$"],
	paste: &[r"(?:^|\s)`([^`\s](?:[^`]*[^`\s])?)`"],
};

/// Inline code: `Mod-e` or `` `text` ``.
///
/// Code usually excludes other formatting. The schema can only exclude
/// marks it knows, so list them in `excludes`; the StarterKit fills it with
/// the marks it registers.
#[derive(Clone, Debug, Default)]
pub struct Code {
	/// Marks that cannot coexist with code.
	pub excludes: Vec<&'static str>,
}

impl Extension for Code {
	fn name(&self) -> &'static str {
		CODE.name
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark(MarkSpec::new("code").excludes(&self.excludes))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		CODE.nodes()
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(CODE.commands())
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(CODE.queries())
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(CODE.keymap())
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		CODE.input_rules(schema)
	}

	fn paste_rules(&self, schema: &Schema) -> Result<Vec<PasteRule>, EditorError> {
		CODE.paste_rules(schema)
	}
}

impl TypedExtension for Code {
	type Handle = MarkHandle;

	fn handle(&self, registration: Registration) -> MarkHandle {
		CODE.handle(registration)
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
			.mark(MarkSpec::new("code"))
			.build()
			.expect("valid schema");

		assert!(CODE.input_rules(&schema).is_ok());
		assert!(CODE.paste_rules(&schema).is_ok());
	}
}
