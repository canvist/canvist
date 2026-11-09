//! Marks that only switch on and off, such as bold or underline.
//!
//! A [`SimpleMark`] describes one: its name, its toggle, set, and unset
//! commands, its active query, its keys, and its Markdown-style input and
//! paste rules. An [`Extension`](crate::Extension) for such a mark adds the
//! mark to the schema and delegates everything else to its description, so
//! extensions differ only in data:
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Binding;
//! use canvist_core::CommandSpec;
//! use canvist_core::Editor;
//! use canvist_core::EditorError;
//! use canvist_core::Env;
//! use canvist_core::Extension;
//! use canvist_core::InputRule;
//! use canvist_core::MarkHandle;
//! use canvist_core::NodeMeta;
//! use canvist_core::Platform;
//! use canvist_core::QuerySpec;
//! use canvist_core::Registration;
//! use canvist_core::SimpleMark;
//! use canvist_core::TypedExtension;
//! use canvist_model::BlockSpec;
//! use canvist_model::MarkSpec;
//! use canvist_model::Schema;
//! use canvist_model::SchemaBuilder;
//!
//! static SPOILER: SimpleMark = SimpleMark {
//! 	name: "spoiler",
//! 	toggle: "toggleSpoiler",
//! 	set: "setSpoiler",
//! 	unset: "unsetSpoiler",
//! 	is_active: "isSpoiler",
//! 	keys: &["Mod-Shift-x"],
//! 	input: &[r"(?:^|\s)\|\|([^|\s](?:[^|]*[^|\s])?)\|\|$"],
//! 	paste: &[],
//! };
//!
//! struct Spoiler;
//!
//! impl Extension for Spoiler {
//! 	fn name(&self) -> &'static str {
//! 		SPOILER.name
//! 	}
//!
//! 	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
//! 		schema
//! 			.block(BlockSpec::text("paragraph"))
//! 			.mark(MarkSpec::new(SPOILER.name))
//! 	}
//!
//! 	fn nodes(&self) -> Vec<NodeMeta> {
//! 		SPOILER.nodes()
//! 	}
//!
//! 	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
//! 		Ok(SPOILER.commands())
//! 	}
//!
//! 	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
//! 		Ok(SPOILER.queries())
//! 	}
//!
//! 	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
//! 		Ok(SPOILER.keymap())
//! 	}
//!
//! 	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
//! 		SPOILER.input_rules(schema)
//! 	}
//! }
//!
//! impl TypedExtension for Spoiler {
//! 	type Handle = MarkHandle;
//!
//! 	fn handle(&self, registration: Registration) -> MarkHandle {
//! 		SPOILER.handle(registration)
//! 	}
//! }
//!
//! let mut builder = Editor::builder();
//! let spoiler = builder.add(Spoiler);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! editor.run(&spoiler.toggle(), &mut env)?;
//! assert!(spoiler.is_active(editor.state()));
//! # Ok::<(), EditorError>(())
//! ```

use canvist_model::Mark;
use canvist_model::Schema;
use canvist_model::Value;

use crate::Binding;
use crate::Command;
use crate::CommandSpec;
use crate::EditorError;
use crate::EditorState;
use crate::InputRule;
use crate::NodeKind;
use crate::NodeMeta;
use crate::PasteRule;
use crate::QuerySpec;
use crate::Registration;
use crate::ValueType;
use crate::command;
use crate::commands;
use crate::extension::mark_type;
use crate::query;

/// Everything that distinguishes one on-or-off mark from another.
#[derive(Debug)]
pub struct SimpleMark {
	/// The mark and extension name, such as `bold`.
	pub name: &'static str,
	/// The command adding the mark, or removing it if every selected
	/// character has it, such as `toggleBold`.
	pub toggle: &'static str,
	/// The command adding the mark, such as `setBold`.
	pub set: &'static str,
	/// The command removing the mark, such as `unsetBold`.
	pub unset: &'static str,
	/// The query reporting whether the mark is active, such as `isBold`.
	pub is_active: &'static str,
	/// The keys that toggle the mark, such as `Mod-b`.
	pub keys: &'static [&'static str],
	/// Input rule patterns, matched as text is typed; group 1 is the text
	/// to format.
	pub input: &'static [&'static str],
	/// Paste rule patterns; group 1 is the text to format.
	pub paste: &'static [&'static str],
}

impl SimpleMark {
	/// Add the mark to the selection, or remove it if every selected
	/// character has it. At a caret, it applies to the next typed text.
	#[must_use]
	pub fn toggle(&'static self) -> Command {
		command(move |state| {
			let kind = mark_type(state.schema(), self.name, self.name)?;
			commands::toggle_mark(state, &Mark::new(kind))
		})
	}

	/// Add the mark to the selection.
	#[must_use]
	pub fn set(&'static self) -> Command {
		command(move |state| {
			let kind = mark_type(state.schema(), self.name, self.name)?;
			commands::set_mark(state, &Mark::new(kind))
		})
	}

	/// Remove the mark from the selection.
	#[must_use]
	pub fn unset(&'static self) -> Command {
		command(move |state| {
			let kind = mark_type(state.schema(), self.name, self.name)?;
			commands::unset_mark(state, kind)
		})
	}

	/// Whether the mark is active at the selection.
	#[must_use]
	pub fn is_active(&self, state: &EditorState) -> bool {
		query::mark_active(state, self.name)
	}

	/// The mark, for [`Extension::nodes`](crate::Extension::nodes).
	#[must_use]
	pub fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Mark, self.name)]
	}

	/// The toggle, set, and unset commands, for
	/// [`Extension::commands`](crate::Extension::commands).
	#[must_use]
	pub fn commands(&'static self) -> Vec<CommandSpec> {
		vec![
			CommandSpec::simple(
				self.toggle,
				"Add the mark to the selection, or remove it if every selected character has it.",
				self.toggle(),
			),
			CommandSpec::simple(self.set, "Add the mark to the selection.", self.set()),
			CommandSpec::simple(
				self.unset,
				"Remove the mark from the selection.",
				self.unset(),
			),
		]
	}

	/// The active query, for [`Extension::queries`](crate::Extension::queries).
	#[must_use]
	pub fn queries(&'static self) -> Vec<QuerySpec> {
		vec![QuerySpec::new(
			self.is_active,
			"Whether the mark is active at the selection.",
			ValueType::Bool,
			move |state, _| Ok(Value::Bool(self.is_active(state))),
		)]
	}

	/// The toggle keys, for [`Extension::keymap`](crate::Extension::keymap).
	#[must_use]
	pub fn keymap(&'static self) -> Vec<Binding> {
		self.keys.iter().map(|&key| (key, self.toggle())).collect()
	}

	/// The input rules, for
	/// [`Extension::input_rules`](crate::Extension::input_rules).
	pub fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let kind = mark_type(schema, self.name, self.name)?;
		self.input
			.iter()
			.map(|pattern| InputRule::mark(pattern, kind))
			.collect()
	}

	/// The paste rules, for
	/// [`Extension::paste_rules`](crate::Extension::paste_rules).
	pub fn paste_rules(&self, schema: &Schema) -> Result<Vec<PasteRule>, EditorError> {
		let kind = mark_type(schema, self.name, self.name)?;
		self.paste
			.iter()
			.map(|pattern| PasteRule::mark(pattern, kind))
			.collect()
	}

	/// The typed handle, for
	/// [`TypedExtension::handle`](crate::TypedExtension::handle).
	#[must_use]
	pub fn handle(&'static self, registration: Registration) -> MarkHandle {
		MarkHandle {
			mark: self,
			_registration: registration,
		}
	}
}

/// Typed access to a [`SimpleMark`] extension, such as bold.
#[derive(Clone, Copy, Debug)]
pub struct MarkHandle {
	mark: &'static SimpleMark,
	_registration: Registration,
}

impl MarkHandle {
	/// Add the mark to the selection, or remove it if every selected
	/// character has it. At a caret, it applies to the next typed text.
	#[must_use]
	pub fn toggle(self) -> Command {
		self.mark.toggle()
	}

	/// Add the mark to the selection.
	#[must_use]
	pub fn set(self) -> Command {
		self.mark.set()
	}

	/// Remove the mark from the selection.
	#[must_use]
	pub fn unset(self) -> Command {
		self.mark.unset()
	}

	/// Whether the mark is active at the selection.
	#[must_use]
	pub fn is_active(self, state: &EditorState) -> bool {
		self.mark.is_active(state)
	}
}
