#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_mention`
//!
//! TipTap's `mention` node for the canvist editor, as in
//! `@tiptap/extension-mention`: `@Ada`, an atomic inline embed with an `id`,
//! a `label`, and the `mentionSuggestionChar` that started it.
//!
//! Typing `@` starts a suggestion, built on
//! [`canvist_extension_suggestion`]: the `mentionSuggestion` query reports
//! what follows it, the host shows matching people, and `insertMention`
//! replaces `@query` with the mention and a space. Escape, or
//! `dismissMentionSuggestion`, dismisses the suggestion, and Backspace
//! right after a mention turns it back into its trigger character. The
//! trigger and whether the query may hold spaces are [`Mention`]'s fields.
//! Its handle is a [`MentionHandle`].
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_mention::Mention;
//! use canvist_extension_mention::MentionTarget;
//! use canvist_extension_paragraph::Paragraph;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let mention = builder.add(Mention::default());
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! for character in "Hi @ad".chars() {
//! 	editor.insert_text(&character.to_string(), &mut env)?;
//! }
//! assert_eq!(mention.suggestion(editor.state()).as_deref(), Some("ad"));
//!
//! let ada = MentionTarget { id: "7", label: Some("Ada") };
//! editor.run(&mention.insert(ada), &mut env)?;
//! assert_eq!(editor.state().doc().plain_text(), "Hi \u{fffc} ", "the mention is an embed");
//! assert_eq!(mention.suggestion(editor.state()), None);
//! # Ok::<(), canvist_core::EditorError>(())
//! ```

use canvist_core::Binding;
use canvist_core::Command;
use canvist_core::CommandResult;
use canvist_core::CommandSpec;
use canvist_core::EditKind;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::Field;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::Platform;
use canvist_core::QuerySpec;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::ValueType;
use canvist_core::command;
use canvist_core::commands;
use canvist_core::embed_type;
use canvist_extension_suggestion::Trigger;
use canvist_model::Attrs;
use canvist_model::Embed;
use canvist_model::EmbedSpec;
use canvist_model::InlineText;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Selection;
use canvist_model::Value;

/// The embed's name, as in TipTap.
const MENTION: &str = "mention";

/// TipTap's `mention` node: an atom with `id`, `label`, and the
/// `mentionSuggestionChar` that started it.
///
/// Typing `@` starts a suggestion: the `mentionSuggestion` query reports
/// what follows it, the host shows matching people, and `insertMention`
/// replaces `@query` with the mention and a space. Escape dismisses the
/// suggestion. Backspace right after a mention turns it back into its
/// trigger character, so the suggestion opens again, as TipTap does by
/// default.
#[derive(Clone, Copy, Debug)]
pub struct Mention {
	/// The character that starts a mention.
	pub char: char,
	/// Whether the query may hold spaces, as `@Ada Lovelace` does.
	pub allow_spaces: bool,
}

impl Default for Mention {
	fn default() -> Self {
		Self {
			char: '@',
			allow_spaces: false,
		}
	}
}

/// A person, team, or anything else a mention points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MentionTarget<'a> {
	/// A stable id, such as a user id.
	pub id: &'a str,
	/// The text shown after the trigger, such as a name.
	pub label: Option<&'a str>,
}

/// Typed access to [`Mention`].
#[derive(Clone, Copy, Debug)]
pub struct MentionHandle {
	config: Mention,
	_registration: Registration,
}

impl MentionHandle {
	/// The query typed after the trigger, if a mention is being suggested.
	#[must_use]
	pub fn suggestion(self, state: &EditorState) -> Option<String> {
		self.config.trigger().find(state).map(|found| found.query)
	}

	/// Replace the suggestion, or insert at the caret, with a mention of
	/// `target` followed by a space.
	#[must_use]
	pub fn insert(self, target: MentionTarget<'_>) -> Command {
		self.config.insert(target.id, target.label)
	}

	/// Stop suggesting until another `@` is typed.
	#[must_use]
	pub fn dismiss(self) -> Command {
		self.config.dismiss()
	}
}

impl Mention {
	const fn trigger(self) -> Trigger {
		Trigger {
			name: MENTION,
			char: self.char,
			allow_spaces: self.allow_spaces,
		}
	}

	fn insert(self, id: &str, label: Option<&str>) -> Command {
		let attrs = Attrs::new()
			.with("id", id)
			.with("label", label.map_or(Value::Null, Value::from))
			.with("mentionSuggestionChar", self.char.to_string());

		command(move |state| {
			let kind = embed_type(state.schema(), MENTION, MENTION)?;
			let marks = commands::active_marks(state);
			let embed = InlineText::embed(
				Embed {
					kind,
					attrs: attrs.clone(),
				},
				marks,
			);
			let found = self.trigger().find(state);
			canvist_extension_suggestion::replace(state, found, embed)
		})
	}

	fn dismiss(self) -> Command {
		command(move |state| self.trigger().dismiss(state))
	}

	/// Backspace right after a mention: replace it with its trigger
	/// character, opening the suggestion again.
	fn backspace(self) -> Command {
		command(move |state| reopen(state, self.char))
	}
}

/// Replace the mention just before the caret with the character that
/// started it, in the mention's marks.
fn reopen(state: &mut EditorState, default_char: char) -> CommandResult {
	let Some(caret) = state.selection().as_caret() else {
		return Ok(None);
	};
	let Some(kind) = state.schema().embed_type(MENTION) else {
		return Ok(None);
	};
	let Some(text) = state.doc().inline(caret.block) else {
		return Ok(None);
	};
	let mention = text
		.runs()
		.find(|run| run.range.end == caret.offset)
		.and_then(|run| {
			let embed = run.embed.filter(|embed| embed.kind == kind)?;
			Some((run.range.start, embed.clone(), run.marks.clone()))
		});
	let Some((start, mention, marks)) = mention else {
		return Ok(None);
	};
	let trigger = mention
		.attrs
		.get("mentionSuggestionChar")
		.and_then(Value::as_str)
		.map_or_else(|| default_char.to_string(), str::to_owned);
	state.change(EditKind::Deleting, |tr| {
		tr.delete_text(caret.block, start..caret.offset)?;
		let end = tr.insert_text(Point::new(caret.block, start), &trigger, marks)?;
		tr.set_selection(Selection::caret(end));
		Ok(true)
	})
}

fn fields() -> [Field; 3] {
	[
		Field::optional(
			"id",
			ValueType::nullable(ValueType::String),
			Value::Null,
			"What the mention points at, such as a user id.",
		),
		Field::optional(
			"label",
			ValueType::nullable(ValueType::String),
			Value::Null,
			"The text shown after the trigger character.",
		),
		Field::optional(
			"mentionSuggestionChar",
			ValueType::String,
			"@",
			"The character that started the mention.",
		),
	]
}

impl Extension for Mention {
	fn name(&self) -> &'static str {
		MENTION
	}

	/// Above the default, so Backspace after a mention reopens it before
	/// the standard keys delete it, as TipTap's priority 101 does.
	fn priority(&self) -> i32 {
		101
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		let attrs = fields()
			.into_iter()
			.map(|field| (field.name, field.default.unwrap_or_default()))
			.collect();
		schema.embed(EmbedSpec::new(MENTION).attrs(attrs))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		let node = NodeMeta::new(NodeKind::Embed, MENTION);
		vec![fields().into_iter().fold(node, NodeMeta::attr)]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let config = *self;
		let insert = CommandSpec::new(
			"insertMention",
			"Replace the mention suggestion, or insert at the caret, with a mention and a space.",
			move |args| Ok(config.insert(args.string("id")?, args.optional_string("label")?)),
		)
		.arg(Field::required(
			"id",
			ValueType::String,
			"What the mention points at.",
		))
		.arg(Field::optional(
			"label",
			ValueType::nullable(ValueType::String),
			Value::Null,
			"The text shown after the trigger character.",
		));

		Ok(vec![
			insert,
			CommandSpec::simple(
				"dismissMentionSuggestion",
				"Stop suggesting mentions until another trigger character is typed.",
				self.dismiss(),
			),
		])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		let trigger = self.trigger();

		Ok(vec![QuerySpec::new(
			"mentionSuggestion",
			"The trigger character and the query typed after it, while a mention is suggested.",
			canvist_extension_suggestion::query_type(),
			move |state, _| Ok(trigger.query_value(state)),
		)])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![
			("Escape", self.dismiss()),
			("Backspace", self.backspace()),
		])
	}
}

impl TypedExtension for Mention {
	type Handle = MentionHandle;

	fn handle(&self, registration: Registration) -> MentionHandle {
		MentionHandle {
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
	use canvist_core::KeyPress;
	use canvist_extension_document::Document;
	use canvist_extension_paragraph::Paragraph;
	use canvist_model::Mark;
	use canvist_model::MarkSpec;

	use super::*;

	/// A `bold` mark and nothing else, to check marks carry over.
	struct Bold;

	impl Extension for Bold {
		fn name(&self) -> &'static str {
			"bold"
		}

		fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
			schema.mark(MarkSpec::new("bold"))
		}
	}

	fn editor(config: Mention) -> Result<(Editor, MentionHandle), EditorError> {
		let mut builder = Editor::builder()
			.extension(Document)
			.extension(Paragraph)
			.extension(Bold);
		let mention = builder.add(config);
		Ok((builder.build()?, mention))
	}

	fn env() -> Env<'static> {
		Env::headless(Duration::ZERO)
	}

	fn type_text(editor: &mut Editor, text: &str) -> Result<(), EditorError> {
		for character in text.chars() {
			editor.insert_text(&character.to_string(), &mut env())?;
		}

		Ok(())
	}

	#[test]
	fn a_configured_trigger_may_allow_spaces() -> Result<(), EditorError> {
		let config = Mention {
			char: '+',
			allow_spaces: true,
		};
		let (mut editor, mention) = editor(config)?;
		type_text(&mut editor, "+Ada Lo")?;

		assert_eq!(
			mention.suggestion(editor.state()).as_deref(),
			Some("Ada Lo")
		);
		Ok(())
	}

	#[test]
	fn escape_dismisses_the_suggestion() -> Result<(), EditorError> {
		let (mut editor, mention) = editor(Mention::default())?;
		type_text(&mut editor, "@ad")?;
		editor.handle_key(KeyPress::plain("Escape"), &mut env())?;

		assert_eq!(mention.suggestion(editor.state()), None);
		Ok(())
	}

	#[test]
	fn backspace_reopens_with_the_mentions_own_trigger_and_marks() -> Result<(), EditorError> {
		let (mut editor, mention) = editor(Mention {
			char: '+',
			allow_spaces: false,
		})?;
		let bold = editor.schema().mark_type("bold").expect("a bold mark");
		let set_bold = command(move |state| commands::set_mark(state, &Mark::new(bold)));
		editor.run(&set_bold, &mut env())?;
		let target = MentionTarget {
			id: "7",
			label: None,
		};
		editor.run(&mention.insert(target), &mut env())?;

		let block = editor.state().doc().children(None)[0];
		let after = Selection::caret(Point::new(
			block,
			canvist_model::EMBED_CHAR.len_utf8() as u32,
		));
		editor.select(after, &mut env())?;

		editor.handle_key(KeyPress::plain("Backspace"), &mut env())?;
		assert_eq!(editor.state().doc().plain_text(), "+ ");
		assert_eq!(mention.suggestion(editor.state()).as_deref(), Some(""));

		let text = editor.state().doc().inline(block).expect("a text block");
		let trigger = text.runs().next().expect("a run");
		assert_eq!(trigger.text, "+");
		assert!(trigger.marks.get(bold).is_some(), "still bold");
		Ok(())
	}
}
