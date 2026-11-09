#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_bold`
//!
//! TipTap's `bold` mark for the canvist editor, as in TipTap's
//! `@tiptap/extension-bold`. [`Bold`] registers the mark with the
//! `toggleBold`, `setBold`, and `unsetBold` commands and the `isBold` query.
//! `Mod-b` toggles it, typing `**text**` or `__text__` makes the text bold,
//! and pasted text gets the same treatment. Its handle is a [`MarkHandle`].
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//! use canvist_extension_bold::Bold;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let bold = builder.add(Bold);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! for character in "**bold**".chars() {
//! 	editor.insert_text(&character.to_string(), &mut env)?;
//! }
//!
//! assert_eq!(editor.state().doc().plain_text(), "bold");
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

static BOLD: SimpleMark = SimpleMark {
	name: "bold",
	toggle: "toggleBold",
	set: "setBold",
	unset: "unsetBold",
	is_active: "isBold",
	keys: &["Mod-b"],
	input: &[
		r"(?:^|\s)\*\*([^*\s](?:[^*]*[^*\s])?)\*\*$",
		r"(?:^|\s)__([^_\s](?:[^_]*[^_\s])?)__$",
	],
	paste: &[
		r"(?:^|\s)\*\*([^*\s](?:[^*]*[^*\s])?)\*\*",
		r"(?:^|\s)__([^_\s](?:[^_]*[^_\s])?)__",
	],
};

/// Bold text: `Mod-b`, `**text**`, or `__text__`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Bold;

impl Extension for Bold {
	fn name(&self) -> &'static str {
		BOLD.name
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark(MarkSpec::new("bold"))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		BOLD.nodes()
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(BOLD.commands())
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(BOLD.queries())
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(BOLD.keymap())
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		BOLD.input_rules(schema)
	}

	fn paste_rules(&self, schema: &Schema) -> Result<Vec<PasteRule>, EditorError> {
		BOLD.paste_rules(schema)
	}
}

impl TypedExtension for Bold {
	type Handle = MarkHandle;

	fn handle(&self, registration: Registration) -> MarkHandle {
		BOLD.handle(registration)
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
			.mark(MarkSpec::new("bold"))
			.build()
			.expect("valid schema");

		assert!(BOLD.input_rules(&schema).is_ok());
		assert!(BOLD.paste_rules(&schema).is_ok());
	}

	/// The text after typing (or pasting) `text`, and whether the last
	/// `selected` in it is bold.
	fn bold(text: &str, paste: bool, selected: &str) -> Result<(String, bool), EditorError> {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		let bold = builder.add(Bold);
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
		Ok((plain, bold.is_active(editor.state())))
	}

	#[test]
	fn typed_delimiters_make_text_bold() -> Result<(), EditorError> {
		assert_eq!(bold("**a b**", false, "a b")?, ("a b".to_owned(), true));
		assert_eq!(bold("x __a__", false, "a")?, ("x a".to_owned(), true));
		Ok(())
	}

	#[test]
	fn delimiters_need_a_word_boundary_and_no_inner_spaces() -> Result<(), EditorError> {
		assert_eq!(
			bold("x** a**", false, "x** a**")?,
			("x** a**".to_owned(), false)
		);
		assert!(!bold("snake__case__", false, "snake__case__")?.1);
		Ok(())
	}

	#[test]
	fn pasted_delimiters_make_text_bold() -> Result<(), EditorError> {
		assert_eq!(bold("a **b** c", true, "b")?, ("a b c".to_owned(), true));
		Ok(())
	}
}
