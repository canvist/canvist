#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_callout`
//!
//! Remirror's `callout` block for the canvist editor, as in
//! `@remirror/extension-callout`: a tinted box for notes, tips, warnings,
//! and errors. [`Callout`] adds the `callout` block, whose `type` attribute
//! the theme colours. TipTap has no callout.
//!
//! `toggleCallout` wraps the selection or lifts it out, `setCalloutType`
//! changes the type, the `calloutType` query reports it, and typing
//! `:::warning ` at the start of a line makes one. Its handle is a
//! [`CalloutHandle`].
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_callout::Callout;
//! use canvist_extension_callout::CalloutType;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let callout = builder.add(Callout);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! for character in ":::warning Mind the gap".chars() {
//! 	editor.insert_text(&character.to_string(), &mut env)?;
//! }
//! assert_eq!(callout.kind(editor.state()), Some(CalloutType::Warning));
//! assert_eq!(editor.state().doc().plain_text(), "Mind the gap");
//!
//! editor.run(&callout.set_type(CalloutType::Success), &mut env)?;
//! assert_eq!(callout.kind(editor.state()), Some(CalloutType::Success));
//! # Ok::<(), canvist_core::EditorError>(())
//! ```

use canvist_core::Args;
use canvist_core::Command;
use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::Field;
use canvist_core::InputRule;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::QuerySpec;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::ValueType;
use canvist_core::block_type;
use canvist_core::command;
use canvist_core::commands;
use canvist_core::query;
use canvist_model::Attrs;
use canvist_model::BlockSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Value;

const CALLOUT: &str = "callout";

/// Remirror's `callout` block: a box of content with a `type` (`info`,
/// `warning`, `error`, `success`, or `blank`) that the theme colours.
///
/// `toggleCallout` wraps the selection or lifts it out, `setCalloutType`
/// changes the type, and `:::warning ` at the start of a line makes one.
/// Markdown writes GitHub's alerts (`> [!WARNING]`) and reads them back.
#[derive(Clone, Copy, Debug, Default)]
pub struct Callout;

/// A callout type for [`CalloutHandle`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CalloutType {
	/// A note: the default.
	#[default]
	Info,
	/// Something to be careful about.
	Warning,
	/// Something that went or will go wrong.
	Error,
	/// Something that worked, or a tip.
	Success,
	/// A plain box with no colour.
	Blank,
}

impl CalloutType {
	/// Every type, in Remirror's order.
	const ALL: [Self; 5] = [
		Self::Info,
		Self::Warning,
		Self::Error,
		Self::Success,
		Self::Blank,
	];

	/// The attribute value, such as `warning`.
	#[must_use]
	pub const fn name(self) -> &'static str {
		match self {
			Self::Info => "info",
			Self::Warning => "warning",
			Self::Error => "error",
			Self::Success => "success",
			Self::Blank => "blank",
		}
	}

	fn from_name(name: &str) -> Option<Self> {
		Self::ALL.into_iter().find(|kind| kind.name() == name)
	}
}

/// Every type's name, for the attribute's type.
fn names() -> Vec<&'static str> {
	CalloutType::ALL.map(CalloutType::name).to_vec()
}

/// Typed access to [`Callout`].
#[derive(Clone, Copy, Debug)]
pub struct CalloutHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl CalloutHandle {
	/// Wrap the selection in a callout of `kind`, or lift it out of the
	/// callout around it.
	#[must_use]
	pub fn toggle(self, kind: CalloutType) -> Command {
		toggle(kind)
	}

	/// Change the type of the callout around the selection.
	#[must_use]
	pub fn set_type(self, kind: CalloutType) -> Command {
		set_type(kind)
	}

	/// The type of the callout around the selection, if there is one.
	#[must_use]
	pub fn kind(self, state: &EditorState) -> Option<CalloutType> {
		type_around(state)
	}
}

/// The type of the callout around the selection, if there is one. A
/// missing type, or one this extension does not know, which JSON or a
/// collaborator may carry, reads as `info`, as the theme draws it.
fn type_around(state: &EditorState) -> Option<CalloutType> {
	let callout = query::around(state, CALLOUT)?;
	let attrs = state.doc().block(callout)?.attrs();
	let named = attrs.get("type").and_then(Value::as_str);

	Some(named.and_then(CalloutType::from_name).unwrap_or_default())
}

fn toggle(kind: CalloutType) -> Command {
	command(move |state| {
		let callout = block_type(state.schema(), CALLOUT, CALLOUT)?;
		commands::toggle_wrap(state, callout, Attrs::new().with("type", kind.name()))
	})
}

fn set_type(kind: CalloutType) -> Command {
	command(move |state| {
		let Some(block) = query::around(state, CALLOUT) else {
			return Ok(None);
		};
		let attrs = state
			.doc()
			.block(block)
			.map(|callout| callout.attrs().clone().with("type", kind.name()))
			.unwrap_or_default();
		commands::set_block_attrs(state, block, &attrs)
	})
}

fn type_arg(args: &Args<'_>) -> Result<CalloutType, EditorError> {
	let name = args.string("type")?;
	CalloutType::from_name(name).ok_or_else(|| {
		EditorError::InvalidArgs {
			name: CALLOUT.to_owned(),
			reason: format!("no callout type `{name}`"),
		}
	})
}

fn type_field(required: bool) -> Field {
	let doc = "The callout type: info, warning, error, success, or blank.";
	let ty = ValueType::OneOf(names());

	if required {
		Field::required("type", ty, doc)
	} else {
		Field::optional("type", ty, "info", doc)
	}
}

impl Extension for Callout {
	fn name(&self) -> &'static str {
		CALLOUT
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.block(
			BlockSpec::container(CALLOUT, &["block"]).attrs(Attrs::new().with("type", "info")),
		)
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Block, CALLOUT).attr(type_field(false))]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let toggle_spec = CommandSpec::new(
			"toggleCallout",
			"Wrap the selection in a callout, or lift it out of the callout around it.",
			|args| Ok(toggle(type_arg(args)?)),
		);
		let set_spec = CommandSpec::new(
			"setCalloutType",
			"Change the type of the callout around the selection.",
			|args| Ok(set_type(type_arg(args)?)),
		);

		Ok(vec![
			toggle_spec.arg(type_field(false)),
			set_spec.arg(type_field(true)),
		])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(vec![QuerySpec::new(
			"calloutType",
			"The type of the callout around the selection, or null outside one.",
			ValueType::nullable(ValueType::OneOf(names())),
			|state, _| Ok(type_around(state).map_or(Value::Null, |kind| Value::from(kind.name()))),
		)])
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let callout = block_type(schema, CALLOUT, CALLOUT)?;
		let rule = InputRule::wrap(r"^:::([a-zA-Z]*)\s$", move |state, found| {
			let named = found.group(1).unwrap_or_default().to_ascii_lowercase();
			let kind = CalloutType::from_name(&named).unwrap_or_default();
			commands::wrap_in(state, callout, Attrs::new().with("type", kind.name()))
		})?;
		Ok(vec![rule])
	}
}

impl TypedExtension for Callout {
	type Handle = CalloutHandle;

	fn handle(&self, registration: Registration) -> CalloutHandle {
		CalloutHandle {
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

	fn typed(text: &str) -> Result<(Editor, CalloutHandle), EditorError> {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		let callout = builder.add(Callout);
		let mut editor = builder.build()?;
		let mut env = Env::headless(Duration::ZERO);

		for character in text.chars() {
			editor.insert_text(&character.to_string(), &mut env)?;
		}

		Ok((editor, callout))
	}

	#[test]
	fn typed_types_ignore_case_and_unknown_ones_are_info() -> Result<(), EditorError> {
		let cases = [
			(":::Error ", Some(CalloutType::Error)),
			("::: ", Some(CalloutType::Info)),
			(":::nonsense ", Some(CalloutType::Info)),
			("a :::warning ", None),
		];

		for (text, kind) in cases {
			let (editor, callout) = typed(text)?;
			assert_eq!(callout.kind(editor.state()), kind, "{text:?}");
		}

		Ok(())
	}

	#[test]
	fn toggling_wraps_and_lifts() -> Result<(), EditorError> {
		let (mut editor, callout) = typed("Note")?;
		let mut env = Env::headless(Duration::ZERO);

		editor.run(&callout.toggle(CalloutType::Blank), &mut env)?;
		assert_eq!(callout.kind(editor.state()), Some(CalloutType::Blank));
		editor.run(&callout.toggle(CalloutType::Blank), &mut env)?;
		assert_eq!(callout.kind(editor.state()), None);
		assert!(
			editor
				.run(&callout.set_type(CalloutType::Error), &mut env)?
				.is_none(),
			"no callout to change"
		);
		Ok(())
	}

	#[test]
	fn an_unknown_stored_type_reads_as_info() -> Result<(), EditorError> {
		let (mut editor, callout) = typed(":::warning ")?;
		let mut env = Env::headless(Duration::ZERO);
		let unknown = command(|state| {
			let block = query::around(state, CALLOUT).expect("a callout");
			commands::set_block_attrs(state, block, &Attrs::new().with("type", "shout"))
		});
		editor.run(&unknown, &mut env)?;

		assert_eq!(callout.kind(editor.state()), Some(CalloutType::Info));
		Ok(())
	}

	#[test]
	fn names_match_the_types() {
		for kind in CalloutType::ALL {
			assert_eq!(CalloutType::from_name(kind.name()), Some(kind));
		}

		assert_eq!(names(), ["info", "warning", "error", "success", "blank"]);
	}
}
