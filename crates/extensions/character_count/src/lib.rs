#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_character_count`
//!
//! Character and word counts, and a character limit, for the canvist editor,
//! as in TipTap's `@tiptap/extension-character-count`. [`CharacterCount`]
//! adds the `characters`, `words`, and `characterLimit` queries and the
//! `setCharacterLimit` command, and with a limit set, refuses edits that would
//! take the document past it. It adds no nodes or marks.
//!
//! Characters are grapheme clusters, not TipTap's UTF-16 code units, so an
//! emoji with a skin tone counts once.
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_character_count::CharacterCount;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let count = builder.add(CharacterCount { limit: Some(8) });
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! editor.insert_text("two words", &mut env)?;
//! assert_eq!(count.characters(editor.state()), 0);
//!
//! editor.insert_text("one word", &mut env)?;
//! assert_eq!(count.characters(editor.state()), 8);
//! assert_eq!(count.words(editor.state()), 2);
//! # Ok::<(), canvist_core::EditorError>(())
//! ```
use canvist_core::Command;
use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::Field;
use canvist_core::QuerySpec;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::Update;
use canvist_core::UpdateFilter;
use canvist_core::ValueType;
use canvist_core::command;
use canvist_core::commands;
use canvist_model::Block;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::EMBED_CHAR;
use canvist_model::InlineText;
use canvist_model::Schema;
use canvist_model::Step;
use canvist_model::TreeContent;
use canvist_model::Value;
use unicode_segmentation::UnicodeSegmentation;

/// The editor setting holding a limit set at run time.
const LIMIT: &str = "characterLimit";

/// Counts characters and words, and with a `limit`, refuses edits that would
/// take the document past it, as TipTap's `CharacterCount` does.
///
/// A character is what a reader sees as one: a grapheme cluster, so `👍🏽`
/// and `é` written as `e` plus an accent each count once. (TipTap counts
/// UTF-16 code units by default.) Inline embeds such as mentions count as
/// none, and blocks are separate words. An edit that would go past the limit
/// does not apply, so a paste that would overflow is refused whole; edits
/// that shrink an over-long document always apply.
#[derive(Clone, Copy, Debug, Default)]
pub struct CharacterCount {
	/// The most characters the document may hold, until
	/// `setCharacterLimit` changes it.
	pub limit: Option<u32>,
}

/// Typed access to [`CharacterCount`].
#[derive(Clone, Copy, Debug)]
pub struct CharacterCountHandle {
	limit: Option<u32>,
	_registration: Registration,
}

impl CharacterCountHandle {
	/// The characters in the document.
	#[must_use]
	pub fn characters(self, state: &EditorState) -> usize {
		characters(state.doc())
	}

	/// The words in the document.
	#[must_use]
	pub fn words(self, state: &EditorState) -> usize {
		words(state.doc())
	}

	/// The limit in force, if there is one.
	#[must_use]
	pub fn limit(self, state: &EditorState) -> Option<u32> {
		limit_in(state, self.limit)
	}

	/// Change the limit, or remove it with `None`.
	#[must_use]
	pub fn set_limit(self, limit: Option<u32>) -> Command {
		set_limit(limit.map_or(Value::Null, |limit| Value::Int(i64::from(limit))))
	}
}

/// The grapheme clusters in a block's text, leaving out embeds. The text is
/// counted whole, so a cluster split across differently marked runs counts
/// once.
fn text_characters(text: &InlineText) -> usize {
	text.as_str()
		.graphemes(true)
		.filter(|grapheme| !grapheme.trim_start_matches(EMBED_CHAR).is_empty())
		.count()
}

/// The words in a block's text. Embeds separate words, but marks do not, so
/// `he**llo**` is one word.
fn text_words(text: &InlineText) -> usize {
	text.as_str()
		.split(|character: char| character.is_whitespace() || character == EMBED_CHAR)
		.filter(|word| !word.is_empty())
		.count()
}

fn characters(doc: &Document) -> usize {
	doc.text_blocks()
		.filter_map(Block::inline)
		.map(text_characters)
		.sum()
}

fn words(doc: &Document) -> usize {
	doc.text_blocks()
		.filter_map(Block::inline)
		.map(text_words)
		.sum()
}

/// The characters in every text block of `tree`.
fn tree_characters(tree: &BlockTree) -> usize {
	let mut count = 0;

	tree.walk(&mut |node: &BlockTree| {
		if let TreeContent::Inline(text) = &node.content {
			count += text_characters(text);
		}
	});

	count
}

/// The characters `steps` insert.
fn inserted(steps: &[Step]) -> usize {
	steps
		.iter()
		.map(|step| {
			match step {
				Step::InsertInline { content, .. } => text_characters(content),
				Step::InsertBlocks { blocks, .. } => blocks.iter().map(tree_characters).sum(),
				_ => 0,
			}
		})
		.sum()
}

/// The limit in force: one set at run time, or the configured default.
fn limit_in(state: &EditorState, configured: Option<u32>) -> Option<u32> {
	match state.settings().get(LIMIT) {
		Some(Value::Int(limit)) => u32::try_from(*limit).ok(),
		Some(_) => None,
		None => configured,
	}
}

/// A `limit` argument: null, or a count that fits `u32`.
fn limit_arg(limit: &Value) -> Result<Value, EditorError> {
	if let Value::Int(count) = limit
		&& u32::try_from(*count).is_err()
	{
		return Err(EditorError::InvalidArgs {
			name: "setCharacterLimit".to_owned(),
			reason: format!("a limit of {count} characters is out of range"),
		});
	}

	Ok(limit.clone())
}

fn set_limit(limit: Value) -> Command {
	command(move |state| commands::set_setting(state, LIMIT, limit.clone()))
}

impl Extension for CharacterCount {
	fn name(&self) -> &'static str {
		"characterCount"
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let spec = CommandSpec::new(
			"setCharacterLimit",
			"Change the character limit, or remove it with null.",
			|args| Ok(set_limit(limit_arg(args.get("limit")?)?)),
		);
		let limit = Field::required(
			"limit",
			ValueType::nullable(ValueType::Int),
			"The most characters the document may hold, from 0 to 4294967295, or null for no \
			 limit.",
		);
		Ok(vec![spec.arg(limit)])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		let configured = self.limit;

		Ok(vec![
			QuerySpec::new(
				"characters",
				"The characters in the document: grapheme clusters, leaving out embeds.",
				ValueType::Int,
				|state, _| Ok(Value::Int(characters(state.doc()) as i64)),
			),
			QuerySpec::new(
				"words",
				"The words in the document.",
				ValueType::Int,
				|state, _| Ok(Value::Int(words(state.doc()) as i64)),
			),
			QuerySpec::new(
				"characterLimit",
				"The character limit in force, or null.",
				ValueType::nullable(ValueType::Int),
				move |state, _| {
					Ok(limit_in(state, configured)
						.map_or(Value::Null, |limit| Value::Int(i64::from(limit))))
				},
			),
		])
	}

	/// Refuse a local edit that adds characters past the limit. Counting
	/// the document is linear, but only runs for edits that add text while a
	/// limit is set, when the document is at most the limit long.
	fn update_filters(&self, _schema: &Schema) -> Result<Vec<UpdateFilter>, EditorError> {
		let configured = self.limit;

		Ok(vec![Box::new(
			move |state: &EditorState, update: &Update| {
				let Some(limit) = limit_in(state, configured) else {
					return true;
				};
				let added = inserted(&update.commit.steps);
				let removed = inserted(&update.commit.inverses);

				added <= removed || characters(state.doc()) <= limit as usize
			},
		)])
	}
}

impl TypedExtension for CharacterCount {
	type Handle = CharacterCountHandle;

	fn handle(&self, registration: Registration) -> CharacterCountHandle {
		CharacterCountHandle {
			limit: self.limit,
			_registration: registration,
		}
	}
}

#[cfg(test)]
mod tests {
	use canvist_model::Attrs;
	use canvist_model::BlockSpec;
	use canvist_model::Embed;
	use canvist_model::EmbedSpec;
	use canvist_model::Mark;
	use canvist_model::MarkSet;
	use canvist_model::MarkSpec;

	use super::*;

	fn schema() -> Schema {
		Schema::builder()
			.block(BlockSpec::text("paragraph"))
			.mark(MarkSpec::new("bold"))
			.embed(EmbedSpec::new("mention"))
			.build()
			.expect("valid schema")
	}

	fn bold(schema: &Schema) -> MarkSet {
		let kind = schema.mark_type("bold").expect("bold");
		MarkSet::from_marks([Mark::new(kind)])
	}

	fn mention(schema: &Schema) -> InlineText {
		let kind = schema.embed_type("mention").expect("mention");
		let embed = Embed {
			kind,
			attrs: Attrs::new(),
		};

		InlineText::embed(embed, MarkSet::empty())
	}

	#[test]
	fn a_word_split_by_marks_counts_once() {
		let schema = schema();
		let mut text = InlineText::plain("he", MarkSet::empty());
		text.append(InlineText::plain("llo world", bold(&schema)));

		assert_eq!(text_words(&text), 2);
		assert_eq!(text_characters(&text), 11);
	}

	#[test]
	fn a_cluster_split_by_marks_counts_once() {
		let schema = schema();
		let mut text = InlineText::plain("e", MarkSet::empty());
		text.append(InlineText::plain("\u{301}", bold(&schema)));

		assert_eq!(text_characters(&text), 1);
	}

	#[test]
	fn embeds_separate_words_and_count_as_no_characters() {
		let schema = schema();
		let mut text = InlineText::plain("hi", MarkSet::empty());
		text.append(mention(&schema));
		text.append(InlineText::plain("there", MarkSet::empty()));

		assert_eq!(text_words(&text), 2);
		assert_eq!(text_characters(&text), 7);
	}

	#[test]
	fn limits_out_of_range_are_refused() {
		assert!(limit_arg(&Value::Int(-1)).is_err());
		assert!(limit_arg(&Value::Int(i64::from(u32::MAX) + 1)).is_err());
		assert_eq!(limit_arg(&Value::Int(5)).ok(), Some(Value::Int(5)));
		assert_eq!(limit_arg(&Value::Null).ok(), Some(Value::Null));
	}
}
