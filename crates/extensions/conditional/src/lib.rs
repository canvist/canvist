#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_conditional`
//!
//! Conditional text for the canvist editor: words that only some versions of
//! a document keep, such as a sentence for one audience or the handle a
//! person goes by on one network. [`Conditional`] adds a `conditional` mark
//! with a `when` attribute, the `setConditional`, `insertConditional`,
//! `toggleConditional`, and `unsetConditional` commands, and the
//! `isConditional` and `conditionalWhen` queries. Neither TipTap nor Remirror
//! has it; it is shaped like their link mark, a non-inclusive mark with one
//! attribute.
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_core::command;
//! use canvist_core::commands;
//! use canvist_extension_conditional::Conditional;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let conditional = builder.add(Conditional);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! editor.insert_text("@canvist", &mut env)?;
//! editor.run(&command(commands::select_all), &mut env)?;
//! editor.run(&conditional.set("instagram"), &mut env)?;
//!
//! assert_eq!(conditional.when(editor.state()).as_deref(), Some("instagram"));
//! # Ok::<(), canvist_core::EditorError>(())
//! ```
use canvist_core::Args;
use canvist_core::Command;
use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::Field;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::QuerySpec;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::ValueType;
use canvist_core::command;
use canvist_core::commands;
use canvist_core::mark_type;
use canvist_core::query;
use canvist_model::Attrs;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::MarkSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Value;

const CONDITIONAL: &str = "conditional";
const WHEN: &str = "when";

/// The `conditional` mark: text that belongs in some versions of the
/// document and not others, such as a sentence for one audience or the
/// handle a person goes by on one network.
///
/// `when` says which versions keep the text. canvist never reads it: it is
/// the host's own string, such as `"instagram threads"`, and the host
/// decides what it means when it renders or exports a version. The editor
/// tints conditional text so writers see it. The mark is not inclusive, so
/// typing after a conditional run writes ordinary text, as after a link:
/// text that leaked into a version by accident would go out unseen. Make a
/// selection conditional with `setConditional`, or write conditional text
/// at the caret with `insertConditional`.
/// HTML writes `<span data-type="conditional" data-when="…">`; Markdown
/// keeps only the text.
#[derive(Clone, Copy, Debug, Default)]
pub struct Conditional;

/// Typed access to [`Conditional`].
#[derive(Clone, Copy, Debug)]
pub struct ConditionalHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl ConditionalHandle {
	/// Make the selected text conditional on `when`. It does not apply at a
	/// caret: [`ConditionalHandle::insert`] writes conditional text there.
	#[must_use]
	pub fn set(self, when: &str) -> Command {
		set(when)
	}

	/// Write `text`, conditional on `when`, over the selection, leaving the
	/// caret after it.
	#[must_use]
	pub fn insert(self, when: &str, text: &str) -> Command {
		insert(when, text)
	}

	/// Make the selection unconditional when it is conditional on `when`;
	/// otherwise make it conditional on `when`.
	#[must_use]
	pub fn toggle(self, when: &str) -> Command {
		toggle(when)
	}

	/// Make the selection unconditional.
	#[must_use]
	pub fn unset(self) -> Command {
		unset()
	}

	/// The `when` every selected character is conditional on, if they share
	/// one.
	#[must_use]
	pub fn when(self, state: &EditorState) -> Option<String> {
		when(state)
	}
}

fn mark(state: &EditorState, when: &str) -> Result<Mark, EditorError> {
	let kind = mark_type(state.schema(), CONDITIONAL, CONDITIONAL)?;
	Ok(Mark::with_attrs(kind, Attrs::new().with(WHEN, when)))
}

fn set(when: &str) -> Command {
	let when = when.to_owned();
	command(move |state| {
		let mark = mark(state, &when)?;

		if state.selection().is_caret() {
			return Ok(None);
		}

		commands::set_mark(state, &mark)
	})
}

fn insert(when: &str, text: &str) -> Command {
	let when = when.to_owned();
	let text = text.to_owned();
	command(move |state| {
		let mark = mark(state, &when)?;

		if text.is_empty() {
			return Ok(None);
		}

		let marks = commands::active_marks(state).with(&mark, state.schema());
		commands::insert_inline(state, InlineText::plain(&text, marks))
	})
}

/// Unconditional when the selection is already conditional on `when`;
/// otherwise conditional on `when`, so toggling another `when` switches to
/// it, as toggling a highlight colour does. Like [`set`], it needs selected
/// text.
fn toggle(when: &str) -> Command {
	let when = when.to_owned();
	command(move |state| {
		let mark = mark(state, &when)?;

		if state.selection().is_caret() {
			return Ok(None);
		}

		if self::when(state).as_deref() == Some(when.as_str()) {
			commands::unset_mark(state, mark.kind)
		} else {
			commands::set_mark(state, &mark)
		}
	})
}

fn unset() -> Command {
	command(|state| {
		let kind = mark_type(state.schema(), CONDITIONAL, CONDITIONAL)?;
		commands::unset_mark(state, kind)
	})
}

fn when(state: &EditorState) -> Option<String> {
	let value = query::shared_mark_attr(state, CONDITIONAL, WHEN)?;
	value.as_str().map(str::to_owned)
}

/// The `when` argument of command `name`, which must say something.
fn when_arg<'a>(args: &Args<'a>, name: &str) -> Result<&'a str, EditorError> {
	let when = args.string(WHEN)?;

	if when.trim().is_empty() {
		return Err(EditorError::InvalidArgs {
			name: name.to_owned(),
			reason: "`when` is empty".to_owned(),
		});
	}

	Ok(when)
}

fn when_field() -> Field {
	Field::required(
		WHEN,
		ValueType::String,
		"Which versions of the document keep the text, in the host's own terms, such as \
		 `instagram threads`.",
	)
}

fn command_specs() -> Vec<CommandSpec> {
	let set_spec = CommandSpec::new(
		"setConditional",
		"Make the selected text conditional: kept only in the versions `when` names.",
		|args| Ok(set(when_arg(args, "setConditional")?)),
	);
	let insert_spec = CommandSpec::new(
		"insertConditional",
		"Write text, kept only in the versions `when` names, over the selection.",
		|args| {
			Ok(insert(
				when_arg(args, "insertConditional")?,
				args.string("text")?,
			))
		},
	);
	let toggle_spec = CommandSpec::new(
		"toggleConditional",
		"Make the selection conditional on `when`, or unconditional when it already is.",
		|args| Ok(toggle(when_arg(args, "toggleConditional")?)),
	);
	let text = Field::required("text", ValueType::String, "The conditional text.");

	vec![
		set_spec.arg(when_field()),
		insert_spec.arg(when_field()).arg(text),
		toggle_spec.arg(when_field()),
		CommandSpec::simple(
			"unsetConditional",
			"Make the selection unconditional.",
			unset(),
		),
	]
}

fn query_specs() -> Vec<QuerySpec> {
	vec![
		QuerySpec::new(
			"isConditional",
			"Whether the selection is conditional.",
			ValueType::Bool,
			|state, _| Ok(Value::Bool(query::mark_active(state, CONDITIONAL))),
		),
		QuerySpec::new(
			"conditionalWhen",
			"The `when` every selected character is conditional on, or null.",
			ValueType::nullable(ValueType::String),
			|state, _| Ok(when(state).map_or(Value::Null, Value::from)),
		),
	]
}

impl Extension for Conditional {
	fn name(&self) -> &'static str {
		CONDITIONAL
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark(
			MarkSpec::new(CONDITIONAL)
				.inclusive(false)
				.attrs(Attrs::new().with(WHEN, "")),
		)
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![
			NodeMeta::new(NodeKind::Mark, CONDITIONAL).attr(Field::optional(
				WHEN,
				ValueType::String,
				"",
				"Which versions of the document keep the text, in the host's own terms.",
			)),
		]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(command_specs())
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(query_specs())
	}
}

impl TypedExtension for Conditional {
	type Handle = ConditionalHandle;

	fn handle(&self, registration: Registration) -> ConditionalHandle {
		ConditionalHandle {
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

	fn editor() -> (Editor, ConditionalHandle) {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		let conditional = builder.add(Conditional);

		(builder.build().expect("valid editor"), conditional)
	}

	fn run(editor: &mut Editor, command: &Command) -> bool {
		let mut env = Env::headless(Duration::ZERO);
		editor.run(command, &mut env).expect("ran").is_some()
	}

	fn select_all(editor: &mut Editor) {
		assert!(run(editor, &command(commands::select_all)));
	}

	#[test]
	fn a_blank_when_is_refused_by_the_command_that_got_it() {
		let (mut editor, _) = editor();
		let mut env = Env::headless(Duration::ZERO);
		let args = Attrs::new().with(WHEN, "  ");
		let refused = editor.run_named("toggleConditional", &args, &mut env);

		assert!(matches!(
			refused,
			Err(EditorError::InvalidArgs { name, .. }) if name == "toggleConditional"
		));
	}

	#[test]
	fn set_and_toggle_need_selected_text() {
		let (mut editor, conditional) = editor();

		assert!(!run(&mut editor, &conditional.set("web")));
		assert!(!run(&mut editor, &conditional.toggle("web")));
		assert!(!run(&mut editor, &conditional.insert("web", "")));
	}

	#[test]
	fn toggling_another_when_switches_to_it() {
		let (mut editor, conditional) = editor();
		let mut env = Env::headless(Duration::ZERO);

		editor.insert_text("text", &mut env).expect("typed");
		select_all(&mut editor);

		assert!(run(&mut editor, &conditional.toggle("web")));
		assert!(run(&mut editor, &conditional.toggle("print")));
		assert_eq!(conditional.when(editor.state()).as_deref(), Some("print"));

		assert!(run(&mut editor, &conditional.toggle("print")));
		assert_eq!(conditional.when(editor.state()), None);
	}
}
