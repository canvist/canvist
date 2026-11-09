#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_trailing_node`
//!
//! A trailing paragraph for the canvist editor, like TipTap's
//! `TrailingNode` from `@tiptap/extensions`. After every change,
//! [`TrailingNode`] adds an empty paragraph (the schema's default text
//! block) to the end of the document if the last top-level block is
//! anything else, so there is always somewhere to click and type after a
//! list, quote, table, or divider. It adds no commands, keys, or rules, and
//! its handle is the plain [`Registration`].
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_blockquote::Blockquote;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//! use canvist_extension_trailing_node::TrailingNode;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let quote = builder.add(Blockquote);
//! builder.add(TrailingNode);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! editor.insert_text("Quoted", &mut env)?;
//! editor.run(&quote.toggle(), &mut env)?;
//!
//! // The quote, then the empty paragraph after it.
//! assert_eq!(editor.state().doc().children(None).len(), 2);
//! assert_eq!(editor.state().doc().plain_text(), "Quoted\n");
//! # Ok::<(), canvist_core::EditorError>(())
//! ```

use canvist_core::CommandResult;
use canvist_core::EditKind;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::Update;
use canvist_model::BlockTree;
use canvist_model::InlineText;

/// Keeps an empty paragraph at the end of the document whenever the last
/// block is anything else, so there is always somewhere to click and type
/// after a list, quote, table, or divider.
#[derive(Clone, Copy, Debug, Default)]
pub struct TrailingNode;

impl Extension for TrailingNode {
	fn name(&self) -> &'static str {
		"trailingNode"
	}

	fn after_update(&self, state: &mut EditorState, _update: &Update) -> CommandResult {
		let doc = state.doc();
		let paragraph = doc.schema().default_text_block();
		let children = doc.children(None);
		let last = children.last().and_then(|&id| doc.block(id));

		if last.is_some_and(|block| block.kind() == paragraph) {
			return Ok(None);
		}

		let index = children.len();
		let selection = state.selection();

		state.change(EditKind::Structure, |tr| {
			let id = tr.allocate_id()?;
			let block = BlockTree::text(id, paragraph, InlineText::new());
			tr.insert_blocks(None, index, vec![block])?;
			tr.set_selection(selection);
			Ok(true)
		})
	}
}

impl TypedExtension for TrailingNode {
	type Handle = Registration;

	fn handle(&self, registration: Registration) -> Registration {
		registration
	}
}

#[cfg(test)]
mod tests {
	use std::time::Duration;

	use canvist_core::Editor;
	use canvist_core::EditorError;
	use canvist_core::Env;
	use canvist_extension_blockquote::Blockquote;
	use canvist_extension_document::Document;
	use canvist_extension_paragraph::Paragraph;

	use super::*;

	#[test]
	fn adds_one_paragraph_after_a_container_and_keeps_the_selection() -> Result<(), EditorError> {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		let quote = builder.add(Blockquote);
		builder.add(TrailingNode);
		let mut editor = builder.build()?;
		let mut env = Env::headless(Duration::ZERO);
		editor.insert_text("Quoted", &mut env)?;

		editor.run(&quote.toggle(), &mut env)?;
		editor.insert_text("!", &mut env)?;

		assert_eq!(editor.state().doc().children(None).len(), 2);
		assert_eq!(editor.state().doc().plain_text(), "Quoted!\n");
		assert!(quote.is_active(editor.state()));
		Ok(())
	}

	#[test]
	fn leaves_a_document_ending_in_a_paragraph_alone() -> Result<(), EditorError> {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		builder.add(TrailingNode);
		let mut editor = builder.build()?;
		let mut env = Env::headless(Duration::ZERO);

		editor.insert_text("Hello", &mut env)?;

		assert_eq!(editor.state().doc().children(None).len(), 1);
		Ok(())
	}
}
