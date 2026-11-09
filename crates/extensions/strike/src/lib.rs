#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_strike`
//!
//! TipTap's `strike` mark for the canvist editor, as in TipTap's
//! `@tiptap/extension-strike`. [`Strike`] registers the mark with the
//! `toggleStrike`, `setStrike`, and `unsetStrike` commands and the
//! `isStrike` query. `Mod-Shift-s` toggles it, typing `~~text~~` strikes the
//! text through, and pasted text gets the same treatment. Its handle is a
//! [`MarkHandle`].
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//! use canvist_extension_strike::Strike;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let strike = builder.add(Strike);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! for character in "~~strike~~".chars() {
//! 	editor.insert_text(&character.to_string(), &mut env)?;
//! }
//!
//! assert_eq!(editor.state().doc().plain_text(), "strike");
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

static STRIKE: SimpleMark = SimpleMark {
	name: "strike",
	toggle: "toggleStrike",
	set: "setStrike",
	unset: "unsetStrike",
	is_active: "isStrike",
	keys: &["Mod-Shift-s"],
	input: &[r"(?:^|\s)~~([^~\s](?:[^~]*[^~\s])?)~~$"],
	paste: &[r"(?:^|\s)~~([^~\s](?:[^~]*[^~\s])?)~~"],
};

/// Struck-through text: `Mod-Shift-s` or `~~text~~`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Strike;

impl Extension for Strike {
	fn name(&self) -> &'static str {
		STRIKE.name
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark(MarkSpec::new("strike"))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		STRIKE.nodes()
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(STRIKE.commands())
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(STRIKE.queries())
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(STRIKE.keymap())
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		STRIKE.input_rules(schema)
	}

	fn paste_rules(&self, schema: &Schema) -> Result<Vec<PasteRule>, EditorError> {
		STRIKE.paste_rules(schema)
	}
}

impl TypedExtension for Strike {
	type Handle = MarkHandle;

	fn handle(&self, registration: Registration) -> MarkHandle {
		STRIKE.handle(registration)
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
			.mark(MarkSpec::new("strike"))
			.build()
			.expect("valid schema");

		assert!(STRIKE.input_rules(&schema).is_ok());
		assert!(STRIKE.paste_rules(&schema).is_ok());
	}

	/// The text after typing (or pasting) `text`, and whether the last
	/// `selected` in it is struck through.
	fn strike(text: &str, paste: bool, selected: &str) -> Result<(String, bool), EditorError> {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		let strike = builder.add(Strike);
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
		Ok((plain, strike.is_active(editor.state())))
	}

	#[test]
	fn typed_delimiters_make_text_struck() -> Result<(), EditorError> {
		assert_eq!(strike("~~a b~~", false, "a b")?, ("a b".to_owned(), true));
		assert_eq!(strike("x ~~a~~", false, "a")?, ("x a".to_owned(), true));
		Ok(())
	}

	#[test]
	fn delimiters_need_a_word_boundary_and_no_inner_spaces() -> Result<(), EditorError> {
		assert_eq!(
			strike("x~~ a~~", false, "x~~ a~~")?,
			("x~~ a~~".to_owned(), false)
		);
		assert!(!strike("a~~b~~", false, "a~~b~~")?.1);
		Ok(())
	}

	#[test]
	fn pasted_delimiters_make_text_struck() -> Result<(), EditorError> {
		assert_eq!(strike("a ~~b~~ c", true, "b")?, ("a b c".to_owned(), true));
		Ok(())
	}
}
