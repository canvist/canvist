#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_placeholder`
//!
//! Hint text in empty blocks for the canvist editor, such as "Write
//! something …", as in TipTap's `@tiptap/extension-placeholder`.
//! [`Placeholder`] adds no nodes or marks: it adds the `setPlaceholder`
//! command and the `placeholder` query, and its handle lists which empty
//! blocks show the text. The `show_only_current` and `include_children`
//! options are TipTap's `showOnlyCurrent` and `includeChildren`.
//!
//! The placeholder is never part of the document: hosts pass
//! [`PlaceholderHandle::placeholders`] to the layout, which draws it.
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//! use canvist_extension_placeholder::Placeholder;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let placeholder = builder.add(Placeholder::default());
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! editor.run(&placeholder.set("Say hello"), &mut env)?;
//! let shown = placeholder.placeholders(editor.state());
//! assert_eq!(shown.first().map(|(_, text)| &**text), Some("Say hello"));
//!
//! editor.insert_text("Hello", &mut env)?;
//! assert!(placeholder.placeholders(editor.state()).is_empty());
//! # Ok::<(), canvist_core::EditorError>(())
//! ```
use std::sync::Arc;

use canvist_core::Command;
use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::Field;
use canvist_core::QuerySpec;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::ValueType;
use canvist_core::command;
use canvist_core::commands;
use canvist_model::Block;
use canvist_model::BlockId;
use canvist_model::InlineText;
use canvist_model::Schema;
use canvist_model::Selection;
use canvist_model::Value;

/// The editor setting holding placeholder text set at run time.
const TEXT: &str = "placeholder";

/// TipTap's default placeholder.
const DEFAULT_TEXT: &str = "Write something …";

/// Hint text shown in empty blocks, such as "Write something …".
///
/// The placeholder is not part of the document. The extension says which
/// empty blocks get it, [`PlaceholderHandle::placeholders`], and hosts pass
/// them to `canvist_layout::DisplayOptions::placeholder`, which draws the
/// text in the block in a faded colour, the same on every platform.
#[derive(Clone, Copy, Debug)]
pub struct Placeholder {
	/// The text, until `setPlaceholder` changes it.
	pub text: &'static str,
	/// Only the empty block holding the caret shows it.
	pub show_only_current: bool,
	/// Empty blocks inside lists, quotes, and other containers show it too,
	/// not only top-level ones.
	pub include_children: bool,
}

impl Default for Placeholder {
	fn default() -> Self {
		Self {
			text: DEFAULT_TEXT,
			show_only_current: true,
			include_children: false,
		}
	}
}

/// Typed access to [`Placeholder`].
#[derive(Clone, Copy, Debug)]
pub struct PlaceholderHandle {
	config: Placeholder,
	_registration: Registration,
}

impl PlaceholderHandle {
	/// The empty blocks that show the placeholder, with its text.
	#[must_use]
	pub fn placeholders(self, state: &EditorState) -> Vec<(BlockId, Arc<str>)> {
		self.config.placeholders(state)
	}

	/// Change the placeholder text.
	#[allow(
		clippy::unused_self,
		reason = "handle methods require proof of registration"
	)]
	#[must_use]
	pub fn set(self, text: &str) -> Command {
		set(text)
	}
}

impl Placeholder {
	/// The text in force: one set at run time, or the configured text.
	fn text_in(&self, state: &EditorState) -> Arc<str> {
		let text = state.settings().get(TEXT).and_then(Value::as_str);
		Arc::from(text.unwrap_or(self.text))
	}

	/// Whether `block` is an empty text block that may show a placeholder.
	fn eligible(&self, state: &EditorState, block: BlockId) -> bool {
		let doc = state.doc();
		let top_level = doc
			.block(block)
			.is_some_and(|block| block.parent().is_none());

		doc.inline(block).is_some_and(InlineText::is_empty) && (self.include_children || top_level)
	}

	fn placeholders(&self, state: &EditorState) -> Vec<(BlockId, Arc<str>)> {
		let text = self.text_in(state);
		let blocks: Vec<BlockId> = if self.show_only_current {
			current_block(state).into_iter().collect()
		} else {
			state.doc().text_blocks().map(Block::id).collect()
		};

		blocks
			.into_iter()
			.filter(|&block| self.eligible(state, block))
			.map(|block| (block, text.clone()))
			.collect()
	}

	/// The placeholder the caret's block shows, if it shows one.
	fn current(&self, state: &EditorState) -> Option<Arc<str>> {
		let block = current_block(state)?;
		self.eligible(state, block).then(|| self.text_in(state))
	}
}

/// The text block holding a caret.
fn current_block(state: &EditorState) -> Option<BlockId> {
	match state.selection() {
		Selection::Text { anchor, head } if anchor == head => Some(head.block),
		_ => None,
	}
}

fn set(text: &str) -> Command {
	let text = Value::from(text);
	command(move |state| commands::set_setting(state, TEXT, text.clone()))
}

impl Extension for Placeholder {
	fn name(&self) -> &'static str {
		"placeholder"
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let spec = CommandSpec::new("setPlaceholder", "Change the placeholder text.", |args| {
			Ok(set(args.string("placeholder")?))
		});
		let text = Field::required("placeholder", ValueType::String, "The placeholder text.");
		Ok(vec![spec.arg(text)])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		let config = *self;

		Ok(vec![QuerySpec::new(
			"placeholder",
			"The placeholder the caret's empty block shows, or null.",
			ValueType::nullable(ValueType::String),
			move |state, _| {
				Ok(config
					.current(state)
					.map_or(Value::Null, |text| Value::from(&*text)))
			},
		)])
	}
}

impl TypedExtension for Placeholder {
	type Handle = PlaceholderHandle;

	fn handle(&self, registration: Registration) -> PlaceholderHandle {
		PlaceholderHandle {
			config: *self,
			_registration: registration,
		}
	}
}

#[cfg(test)]
mod tests {
	use std::time::Duration;

	use canvist_core::Editor;
	use canvist_core::Env;
	use canvist_extension_document::Document;
	use canvist_extension_paragraph::Paragraph;

	use super::*;

	/// An editor with two empty paragraphs and the caret in the second.
	fn two_empty_paragraphs(config: Placeholder) -> (Editor, PlaceholderHandle) {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		let placeholder = builder.add(config);
		let mut editor = builder.build().expect("valid editor");
		let mut env = Env::headless(Duration::ZERO);

		editor
			.run(&command(commands::split_block), &mut env)
			.expect("split");

		(editor, placeholder)
	}

	#[test]
	fn only_the_caret_block_shows_it_by_default() {
		let (editor, placeholder) = two_empty_paragraphs(Placeholder::default());
		let shown = placeholder.placeholders(editor.state());

		assert_eq!(shown.len(), 1);
		assert_eq!(Some(shown[0].0), current_block(editor.state()));
		assert_eq!(&*shown[0].1, DEFAULT_TEXT);
	}

	#[test]
	fn every_empty_block_shows_it_when_asked() {
		let config = Placeholder {
			show_only_current: false,
			..Placeholder::default()
		};
		let (editor, placeholder) = two_empty_paragraphs(config);

		assert_eq!(placeholder.placeholders(editor.state()).len(), 2);
	}

	#[test]
	fn a_text_selection_shows_none() {
		let (mut editor, placeholder) = two_empty_paragraphs(Placeholder::default());
		let mut env = Env::headless(Duration::ZERO);

		editor
			.run(&command(commands::select_all), &mut env)
			.expect("selected");

		assert!(placeholder.placeholders(editor.state()).is_empty());
		assert_eq!(Placeholder::default().current(editor.state()), None);
	}
}
