#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_italic`
//!
//! TipTap's `italic` mark for the canvist editor, as in TipTap's
//! `@tiptap/extension-italic`. [`Italic`] registers the mark with the
//! `toggleItalic`, `setItalic`, and `unsetItalic` commands and the
//! `isItalic` query. `Mod-i` toggles it, typing `*text*` or `_text_` makes
//! the text italic, and pasted text gets the same treatment. Its handle is a
//! [`MarkHandle`].
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//! use canvist_extension_italic::Italic;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let italic = builder.add(Italic);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! for character in "*italic*".chars() {
//! 	editor.insert_text(&character.to_string(), &mut env)?;
//! }
//!
//! assert_eq!(editor.state().doc().plain_text(), "italic");
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

static ITALIC: SimpleMark = SimpleMark {
	name: "italic",
	toggle: "toggleItalic",
	set: "setItalic",
	unset: "unsetItalic",
	is_active: "isItalic",
	keys: &["Mod-i"],
	input: &[
		r"(?:^|\s)\*([^*\s](?:[^*]*[^*\s])?)\*$",
		r"(?:^|\s)_([^_\s](?:[^_]*[^_\s])?)_$",
	],
	paste: &[
		r"(?:^|\s)\*([^*\s](?:[^*]*[^*\s])?)\*",
		r"(?:^|\s)_([^_\s](?:[^_]*[^_\s])?)_",
	],
};

/// Italic text: `Mod-i`, `*text*`, or `_text_`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Italic;

impl Extension for Italic {
	fn name(&self) -> &'static str {
		ITALIC.name
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark(MarkSpec::new("italic"))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		ITALIC.nodes()
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(ITALIC.commands())
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(ITALIC.queries())
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(ITALIC.keymap())
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		ITALIC.input_rules(schema)
	}

	fn paste_rules(&self, schema: &Schema) -> Result<Vec<PasteRule>, EditorError> {
		ITALIC.paste_rules(schema)
	}
}

impl TypedExtension for Italic {
	type Handle = MarkHandle;

	fn handle(&self, registration: Registration) -> MarkHandle {
		ITALIC.handle(registration)
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
			.mark(MarkSpec::new("italic"))
			.build()
			.expect("valid schema");

		assert!(ITALIC.input_rules(&schema).is_ok());
		assert!(ITALIC.paste_rules(&schema).is_ok());
	}

	/// The text after typing (or pasting) `text`, and whether the last
	/// `selected` in it is italic.
	fn italic(text: &str, paste: bool, selected: &str) -> Result<(String, bool), EditorError> {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		let italic = builder.add(Italic);
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
		Ok((plain, italic.is_active(editor.state())))
	}

	#[test]
	fn typed_delimiters_make_text_italic() -> Result<(), EditorError> {
		assert_eq!(italic("*a b*", false, "a b")?, ("a b".to_owned(), true));
		assert_eq!(italic("x _a_", false, "a")?, ("x a".to_owned(), true));
		Ok(())
	}

	#[test]
	fn delimiters_need_a_word_boundary_and_no_inner_spaces() -> Result<(), EditorError> {
		assert_eq!(
			italic("x* a*", false, "x* a*")?,
			("x* a*".to_owned(), false)
		);
		assert!(!italic("snake_case_", false, "snake_case_")?.1);
		Ok(())
	}

	#[test]
	fn pasted_delimiters_make_text_italic() -> Result<(), EditorError> {
		assert_eq!(italic("a *b* c", true, "b")?, ("a b c".to_owned(), true));
		Ok(())
	}
}
