#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_history`
//!
//! Undo and redo for the canvist editor, like TipTap's `UndoRedo`
//! extension (once `@tiptap/extension-history`). [`History`] registers the
//! `undo` and `redo` commands and binds `Mod-z` to undo and `Mod-Shift-z`
//! and `Mod-y` to redo. The history itself belongs to the editor (see
//! [`canvist_core::History`]), so the same keys drive a collaborative
//! history.
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_history::History;
//! use canvist_extension_paragraph::Paragraph;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let history = builder.add(History);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! editor.insert_text("Hello", &mut env)?;
//! editor.run(&history.undo(), &mut env)?;
//! assert_eq!(editor.state().doc().plain_text(), "");
//!
//! editor.run(&history.redo(), &mut env)?;
//! assert_eq!(editor.state().doc().plain_text(), "Hello");
//! # Ok::<(), canvist_core::EditorError>(())
//! ```

use std::sync::Arc;

use canvist_core::Binding;
use canvist_core::Command;
use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::Extension;
use canvist_core::Platform;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_model::Schema;

/// `Mod-z` undoes; `Mod-Shift-z` and `Mod-y` redo. The history itself
/// belongs to the editor (see [`canvist_core::History`]), so a
/// collaborative history works with these keys too.
#[derive(Clone, Copy, Debug, Default)]
pub struct History;

/// Typed access to [`History`]. Ask the editor whether there is anything to
/// undo with [`Editor::can_undo`](canvist_core::Editor::can_undo).
#[derive(Clone, Copy, Debug)]
pub struct HistoryHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl HistoryHandle {
	/// Undo the last group of changes.
	#[must_use]
	pub fn undo(self) -> Command {
		undo()
	}

	/// Redo the last undone group.
	#[must_use]
	pub fn redo(self) -> Command {
		redo()
	}
}

fn undo() -> Command {
	Arc::new(canvist_core::undo)
}

fn redo() -> Command {
	Arc::new(canvist_core::redo)
}

impl Extension for History {
	fn name(&self) -> &'static str {
		"history"
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(vec![
			CommandSpec::simple("undo", "Undo the last group of changes.", undo()),
			CommandSpec::simple("redo", "Redo the last undone group.", redo()),
		])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![
			("Mod-z", undo()),
			("Mod-Shift-z", redo()),
			("Mod-y", redo()),
		])
	}
}

impl TypedExtension for History {
	type Handle = HistoryHandle;

	fn handle(&self, registration: Registration) -> HistoryHandle {
		HistoryHandle {
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
	fn keys_undo_and_redo_on_each_platform() -> Result<(), EditorError> {
		let ctrl = Modifiers {
			ctrl: true,
			..Modifiers::NONE
		};
		let cmd = Modifiers {
			meta: true,
			..Modifiers::NONE
		};

		for (platform, held) in [(Platform::Other, ctrl), (Platform::Apple, cmd)] {
			let mut editor = Editor::builder()
				.extension(Document)
				.extension(Paragraph)
				.extension(History)
				.platform(platform)
				.build()?;
			let mut env = Env::headless(Duration::ZERO);
			let shifted = Modifiers {
				shift: true,
				..held
			};
			editor.insert_text("Hello", &mut env)?;

			editor.handle_key(KeyPress::new("z", held), &mut env)?;
			assert_eq!(editor.state().doc().plain_text(), "", "{platform:?}");
			editor.handle_key(KeyPress::new("z", shifted), &mut env)?;
			assert_eq!(editor.state().doc().plain_text(), "Hello", "{platform:?}");
			editor.handle_key(KeyPress::new("z", held), &mut env)?;
			editor.handle_key(KeyPress::new("y", held), &mut env)?;
			assert_eq!(editor.state().doc().plain_text(), "Hello", "{platform:?}");
		}

		Ok(())
	}
}
