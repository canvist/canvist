#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_hard_break`
//!
//! TipTap's `hardBreak` node for the canvist editor, as in TipTap's
//! `@tiptap/extension-hard-break`. [`HardBreak`] registers an inline embed
//! for a line break that stays inside its block, like `<br>`, and the
//! `setHardBreak` command. `Shift-Enter` and `Mod-Enter` insert one. The
//! break carries the marks active at the caret, so formatting continues on
//! the next line, as with TipTap's `keepMarks`.
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_hard_break::HardBreak;
//! use canvist_extension_paragraph::Paragraph;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let hard_break = builder.add(HardBreak);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! editor.insert_text("one", &mut env)?;
//! editor.run(&hard_break.insert(), &mut env)?;
//! editor.insert_text("two", &mut env)?;
//!
//! // One block, with the break shown as the object replacement character.
//! assert_eq!(editor.state().doc().children(None).len(), 1);
//! assert_eq!(editor.state().doc().plain_text(), "one\u{fffc}two");
//! # Ok::<(), canvist_core::EditorError>(())
//! ```

use canvist_core::Binding;
use canvist_core::Command;
use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::Extension;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::Platform;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::command;
use canvist_core::commands;
use canvist_core::embed_type;
use canvist_model::Attrs;
use canvist_model::Embed;
use canvist_model::EmbedSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;

/// A `hardBreak` inline embed: a line break that stays inside its block.
/// `Shift-Enter` and `Mod-Enter` insert one.
#[derive(Clone, Copy, Debug, Default)]
pub struct HardBreak;

/// Typed access to [`HardBreak`].
#[derive(Clone, Copy, Debug)]
pub struct HardBreakHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl HardBreakHandle {
	/// Insert a line break at the selection.
	#[must_use]
	pub fn insert(self) -> Command {
		insert()
	}
}

fn insert() -> Command {
	command(|state| {
		let kind = embed_type(state.schema(), "hardBreak", "hardBreak")?;
		let embed = Embed {
			kind,
			attrs: Attrs::new(),
		};
		commands::insert_embed(state, embed)
	})
}

impl Extension for HardBreak {
	fn name(&self) -> &'static str {
		"hardBreak"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.embed(EmbedSpec::new("hardBreak"))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Embed, "hardBreak")]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(vec![CommandSpec::simple(
			"setHardBreak",
			"Insert a line break at the selection.",
			insert(),
		)])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![("Shift-Enter", insert()), ("Mod-Enter", insert())])
	}
}

impl TypedExtension for HardBreak {
	type Handle = HardBreakHandle;

	fn handle(&self, registration: Registration) -> HardBreakHandle {
		HardBreakHandle {
			_registration: registration,
		}
	}
}

#[cfg(test)]
mod tests {
	use std::time::Duration;

	use canvist_core::Editor;
	use canvist_core::Env;
	use canvist_core::KeyPress;
	use canvist_core::Modifiers;
	use canvist_extension_document::Document;
	use canvist_extension_paragraph::Paragraph;

	use super::*;

	#[test]
	fn shift_and_mod_enter_insert_a_break() -> Result<(), EditorError> {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		builder.add(HardBreak);
		let mut editor = builder.build()?;
		let mut env = Env::headless(Duration::ZERO);
		let ctrl = Modifiers {
			ctrl: true,
			..Modifiers::NONE
		};

		editor.handle_key(KeyPress::new("Enter", Modifiers::SHIFT), &mut env)?;
		editor.handle_key(KeyPress::new("Enter", ctrl), &mut env)?;

		assert_eq!(editor.state().doc().children(None).len(), 1);
		assert_eq!(editor.state().doc().plain_text(), "\u{fffc}\u{fffc}");
		Ok(())
	}
}
