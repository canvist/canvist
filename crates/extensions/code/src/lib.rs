#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_code`
//!
//! TipTap's inline `code` mark for the canvist editor, as in TipTap's
//! `@tiptap/extension-code`. [`Code`] registers the mark with the
//! `toggleCode`, `setCode`, and `unsetCode` commands and the `isCode` query.
//! `Mod-e` toggles it, typing `` `text` `` makes the text code, and pasted
//! text gets the same treatment. Marks named in [`Code::excludes`] cannot
//! coexist with it. Its handle is a [`MarkHandle`].
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
	use std::time::Duration;

	use canvist_core::Editor;
	use canvist_core::Env;
	use canvist_extension_document::Document;
	use canvist_extension_paragraph::Paragraph;
	use canvist_model::BlockSpec;
	use canvist_model::Point;
	use canvist_model::Selection;

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

	/// The text after typing (or pasting) `text`, and whether the last
	/// `selected` in it is code.
	fn code(text: &str, paste: bool, selected: &str) -> Result<(String, bool), EditorError> {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		let code = builder.add(Code::default());
		let mut editor = builder.build()?;
		let mut env = Env::headless(Duration::ZERO);

		if paste {
			editor.paste_text(text, &mut env)?;
		} else {
			for character in text.chars() {
				editor.insert_text(&character.to_string(), &mut env)?;
			}
		}

		let block = editor.state().doc().children(None)[0];
		let plain = editor.state().doc().plain_text();
		let start = plain.rfind(selected).expect("the text holds the selection");
		let point = |at: usize| Point::new(block, u32::try_from(at).expect("a short text"));
		let selection = Selection::text(point(start), point(start + selected.len()));
		editor.select(selection, &mut env)?;
		Ok((plain, code.is_active(editor.state())))
	}

	#[test]
	fn typed_delimiters_make_text_code() -> Result<(), EditorError> {
		assert_eq!(code("`a b`", false, "a b")?, ("a b".to_owned(), true));
		assert_eq!(code("x `a`", false, "a")?, ("x a".to_owned(), true));
		Ok(())
	}

	#[test]
	fn delimiters_need_a_word_boundary_and_no_inner_spaces() -> Result<(), EditorError> {
		assert_eq!(code("x` a`", false, "x` a`")?, ("x` a`".to_owned(), false));
		assert!(!code("a`b`", false, "a`b`")?.1);
		Ok(())
	}

	#[test]
	fn pasted_delimiters_make_text_code() -> Result<(), EditorError> {
		assert_eq!(code("a `b` c", true, "b")?, ("a b c".to_owned(), true));
		Ok(())
	}

	#[test]
	fn excludes_the_listed_marks() {
		let code = Code {
			excludes: vec!["bold"],
		};
		let schema = Schema::builder()
			.block(BlockSpec::text("paragraph"))
			.mark(MarkSpec::new("bold"));
		let schema = code.schema(schema).build().expect("valid schema");
		let bold = schema.mark_type("bold").expect("bold is registered");
		let code = schema.mark_type("code").expect("code is registered");

		assert!(schema.mark(code).excludes.contains(&bold));
	}
}
