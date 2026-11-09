#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_suggestion`
//!
//! Suggestions: the text a trigger character starts before the caret, such
//! as `@ad` while typing a mention, which hosts show a popup for.
//!
//! This is the shared engine behind the mention, hashtag, and emoji
//! extensions, like TipTap's `@tiptap/suggestion` and Remirror's
//! `prosemirror-suggest`. It registers nothing itself. An extension
//! describes its trigger with a [`Trigger`], then builds on three calls:
//!
//! - [`Trigger::find`] reports the [`Suggestion`] the caret is in, and
//!   [`Trigger::query_value`] reports it as the value of a `…Suggestion`
//!   query whose type is [`query_type`].
//! - [`Trigger::dismiss`] stops suggesting at that trigger, usually bound
//!   to Escape, until another trigger starts.
//! - [`replace`] swaps the suggestion for what the user picked, followed by
//!   a space.
//!
//! A suggestion starts at the trigger character after white space or at the
//! start of a text block, never in a code block, and runs to a collapsed
//! caret at most 64 bytes on. Drawing the popup and fetching its items stays
//! with the host, which positions it at the caret.
//!
//! ```
//! use std::sync::Arc;
//!
//! use canvist_core::EditorState;
//! use canvist_core::commands;
//! use canvist_extension_suggestion::Trigger;
//! use canvist_model::BlockSpec;
//! use canvist_model::Document;
//! use canvist_model::InlineText;
//! use canvist_model::MarkSet;
//! use canvist_model::Schema;
//!
//! const MENTION: Trigger = Trigger { name: "mention", char: '@', allow_spaces: false };
//!
//! let schema = Arc::new(Schema::builder().block(BlockSpec::text("paragraph")).build()?);
//! let mut state = EditorState::new(Document::new(schema, 1))?;
//! commands::insert_text(&mut state, "Hi @ad")?;
//!
//! let found = MENTION.find(&state).expect("a suggestion");
//! assert_eq!(found.query, "ad");
//! assert_eq!(found.range, 3..6);
//!
//! let picked = InlineText::plain("@Ada", MarkSet::empty());
//! canvist_extension_suggestion::replace(&mut state, Some(found), picked)?;
//! assert_eq!(state.doc().plain_text(), "Hi @Ada ");
//! assert!(MENTION.find(&state).is_none());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::ops::Range;

use canvist_core::CommandResult;
use canvist_core::EditKind;
use canvist_core::EditorState;
use canvist_core::Field;
use canvist_core::ValueType;
use canvist_core::commands;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::EMBED_CHAR;
use canvist_model::InlineText;
use canvist_model::Point;
use canvist_model::Selection;
use canvist_model::Step;
use canvist_model::Value;

/// How far before the caret a suggestion may start, in bytes, trigger
/// included. Queries are short, and the bound keeps every keystroke's check
/// cheap however long the paragraph is.
const MAX_SUGGESTION: usize = 64;

/// The text block type suggestions never start in.
const CODE_BLOCK: &str = "codeBlock";

/// What starts a suggestion and what its query may hold. A `const` of this
/// type is all an extension keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Trigger {
	/// The extension the trigger belongs to, such as `mention`. It also
	/// names the editor setting, `mentionDismissed`, that remembers a
	/// dismissed suggestion, so two triggers with one name share it.
	pub name: &'static str,
	/// The character that starts it, such as `@`.
	pub char: char,
	/// Whether the query may hold spaces, as `@Ada Lovelace` does. It never
	/// holds the trigger character or an embed.
	pub allow_spaces: bool,
}

/// An active suggestion, as [`Trigger::find`] reports it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Suggestion {
	/// The text block it is in.
	pub block: BlockId,
	/// The trigger character and the query, as byte offsets in the block.
	/// It ends at the caret.
	pub range: Range<u32>,
	/// The text after the trigger character, up to the caret.
	pub query: String,
}

impl Trigger {
	/// The editor setting remembering where a dismissed suggestion started.
	fn dismissed_key(self) -> String {
		format!("{}Dismissed", self.name)
	}

	/// The suggestion the caret is in, unless it was dismissed. There is
	/// none for a range selection or a caret in a code block.
	pub fn find(self, state: &EditorState) -> Option<Suggestion> {
		let found = self.find_any(state)?;
		let dismissed = state.settings().get(&self.dismissed_key());

		(dismissed != Some(&Value::from(where_starts(&found)))).then_some(found)
	}

	/// The suggestion the caret is in, dismissed or not.
	fn find_any(self, state: &EditorState) -> Option<Suggestion> {
		let caret = state.selection().as_caret()?;

		if in_code(state, caret.block) {
			return None;
		}

		let text = state.doc().inline(caret.block)?.as_str();
		let before = text.get(..caret.offset as usize)?;
		let start = self.trigger_start(before)?;

		Some(Suggestion {
			block: caret.block,
			range: start as u32..caret.offset,
			query: before[start + self.char.len_utf8()..].to_owned(),
		})
	}

	/// Where in `before`, the text before the caret, the suggestion starts:
	/// the last trigger character in the final [`MAX_SUGGESTION`] bytes,
	/// when white space or the block's start comes before it and only query
	/// characters after it.
	fn trigger_start(self, before: &str) -> Option<usize> {
		let mut window = before.len().saturating_sub(MAX_SUGGESTION);

		while !before.is_char_boundary(window) {
			window += 1;
		}

		let start = window + before[window..].rfind(self.char)?;
		let query = &before[start + self.char.len_utf8()..];
		let prefix = before[..start].chars().next_back();
		let valid_prefix = prefix.is_none_or(char::is_whitespace);
		let valid_query = query.chars().all(|character| {
			character != EMBED_CHAR && (self.allow_spaces || !character.is_whitespace())
		});

		(valid_prefix && valid_query).then_some(start)
	}

	/// Stop suggesting at the current trigger until another one starts,
	/// such as one typed after a space or in another block. Does not apply
	/// when there is no suggestion, so Escape falls through to its standard
	/// command.
	pub fn dismiss(self, state: &mut EditorState) -> CommandResult {
		let Some(found) = self.find(state) else {
			return Ok(None);
		};

		commands::set_setting(
			state,
			&self.dismissed_key(),
			Value::from(where_starts(&found)),
		)
	}

	/// The answer to the extension's `…Suggestion` query: a record of the
	/// trigger character (`char`) and the query (`query`), or null when
	/// there is no suggestion. Its type is [`query_type`].
	pub fn query_value(self, state: &EditorState) -> Value {
		self.find(state).map_or(Value::Null, |found| {
			let record = Attrs::new()
				.with("char", self.char.to_string())
				.with("query", found.query);
			Value::from(record)
		})
	}
}

/// The type of [`Trigger::query_value`], for the extension's `QuerySpec`.
pub fn query_type() -> ValueType {
	ValueType::nullable(ValueType::Record(vec![
		Field::required("char", ValueType::String, "The trigger character."),
		Field::required("query", ValueType::String, "The text typed after it."),
	]))
}

fn in_code(state: &EditorState, block: BlockId) -> bool {
	let Some(block) = state.doc().block(block) else {
		return false;
	};

	&*state.schema().block(block.kind()).name == CODE_BLOCK
}

fn where_starts(found: &Suggestion) -> String {
	format!("{}@{}", found.block, found.range.start)
}

/// Replace the suggestion (or insert at the caret, when there is none) with
/// `content`, then a space unless one already follows, and leave the caret
/// after it, as TipTap's mention command does. `content` keeps its own
/// marks, and the space takes those where the suggestion started. Does not
/// apply to a range selection.
pub fn replace(
	state: &mut EditorState,
	found: Option<Suggestion>,
	content: InlineText,
) -> CommandResult {
	let Some(caret) = state.selection().as_caret() else {
		return Ok(None);
	};
	let (block, range) = found.map_or((caret.block, caret.offset..caret.offset), |found| {
		(found.block, found.range)
	});
	let marks = commands::marks_at(state.doc(), Point::new(block, range.start));
	let spaced = state
		.doc()
		.inline(block)
		.is_some_and(|text| text.as_str()[range.end as usize..].starts_with(' '));

	state.change(EditKind::Typing, |tr| {
		tr.delete_text(block, range.clone())?;
		let mut end = Point::new(block, range.start + content.len());
		tr.step(Step::InsertInline {
			block,
			offset: range.start,
			content,
		})?;

		if spaced {
			end.offset += 1;
		} else {
			end = tr.insert_text(end, " ", marks)?;
		}

		tr.set_selection(Selection::caret(end));
		Ok(true)
	})
}

#[cfg(test)]
mod tests {
	use std::sync::Arc;

	use canvist_model::BlockSpec;
	use canvist_model::Document;
	use canvist_model::Embed;
	use canvist_model::EmbedSpec;
	use canvist_model::MarkSet;
	use canvist_model::Schema;

	use super::*;

	const MENTION: Trigger = Trigger {
		name: "mention",
		char: '@',
		allow_spaces: false,
	};

	/// A state whose only block, of type `block`, holds `text` with the
	/// caret at its end.
	fn typed_in(block: &'static str, text: &str) -> EditorState {
		let schema = Schema::builder()
			.block(BlockSpec::text(block))
			.build()
			.expect("the schema builds");
		let mut state = EditorState::new(Document::new(Arc::new(schema), 1)).expect("a state");
		commands::insert_text(&mut state, text).expect("text inserts");
		state
	}

	fn typed(text: &str) -> EditorState {
		typed_in("paragraph", text)
	}

	fn query(trigger: Trigger, text: &str) -> Option<String> {
		trigger.find(&typed(text)).map(|found| found.query)
	}

	#[test]
	fn a_trigger_needs_white_space_or_the_block_start_before_it() {
		assert_eq!(query(MENTION, "@"), Some(String::new()));
		assert_eq!(query(MENTION, "hi @ad"), Some("ad".to_owned()));
		assert_eq!(query(MENTION, "hi\t@ad"), Some("ad".to_owned()));
		assert_eq!(query(MENTION, "a@b"), None);
		assert_eq!(query(MENTION, "@a@b"), None, "the last trigger counts");
		assert_eq!(query(MENTION, "@ad min"), None);
		assert_eq!(query(MENTION, "plain"), None);
	}

	#[test]
	fn spaces_end_a_query_unless_allowed() {
		let spaced = Trigger {
			allow_spaces: true,
			..MENTION
		};

		assert_eq!(query(spaced, "@Ada Love"), Some("Ada Love".to_owned()));
		assert_eq!(query(spaced, "@Ada @Gr"), Some("Gr".to_owned()));
	}

	/// A state holding `before`, an embed, then `after`, with the caret at
	/// the end.
	fn around_embed(before: &str, after: &str) -> EditorState {
		let schema = Schema::builder()
			.block(BlockSpec::text("paragraph"))
			.embed(EmbedSpec::new("mention"))
			.build()
			.expect("the schema builds");
		let kind = schema.embed_type("mention").expect("an embed type");
		let mut state = EditorState::new(Document::new(Arc::new(schema), 1)).expect("a state");
		let embed = Embed {
			kind,
			attrs: Attrs::new(),
		};
		commands::insert_text(&mut state, before).expect("text inserts");
		commands::insert_embed(&mut state, embed).expect("the embed inserts");
		commands::insert_text(&mut state, after).expect("text inserts");
		state
	}

	#[test]
	fn an_embed_ends_a_query() {
		assert!(MENTION.find(&around_embed("@a", "b")).is_none());
		assert!(
			MENTION.find(&around_embed("", "@a")).is_none(),
			"an embed is no white space"
		);
		assert!(MENTION.find(&around_embed("", " @a")).is_some());
	}

	#[test]
	fn ranges_and_queries_count_bytes_of_multi_byte_text() {
		let state = typed("é😀 @ü");
		let found = MENTION.find(&state).expect("a suggestion");

		assert_eq!(found.query, "ü");
		assert_eq!(found.range, 7..10);

		let wide = Trigger {
			char: '＠',
			..MENTION
		};
		let found = wide.find(&typed("x ＠ad")).expect("a suggestion");
		assert_eq!(found.query, "ad");
		assert_eq!(found.range, 2..7, "the trigger is three bytes");
	}

	#[test]
	fn a_suggestion_starts_at_most_64_bytes_back() {
		let longest = "x".repeat(MAX_SUGGESTION - 1);
		assert_eq!(
			query(MENTION, &format!("@{longest}")),
			Some(longest.clone())
		);
		assert_eq!(query(MENTION, &format!("@{longest}x")), None);

		let multi_byte = format!("@{}", "é".repeat(MAX_SUGGESTION / 2));
		assert_eq!(
			query(MENTION, &multi_byte),
			None,
			"a window starting inside a character starts after it"
		);
	}

	#[test]
	fn the_window_edge_is_not_the_block_start() {
		let longest = "x".repeat(MAX_SUGGESTION - 1);

		assert_eq!(
			query(MENTION, &format!("a@{longest}")),
			None,
			"the `a` before the trigger still counts"
		);
		assert_eq!(query(MENTION, &format!(" @{longest}")), Some(longest));
	}

	#[test]
	fn code_blocks_and_range_selections_never_suggest() {
		assert!(MENTION.find(&typed_in(CODE_BLOCK, "@ad")).is_none());

		let mut state = typed("@ad");
		let block = state.doc().children(None)[0];
		let range = Selection::text(Point::new(block, 1), Point::new(block, 3));
		commands::set_selection(&mut state, range).expect("selects");
		assert!(MENTION.find(&state).is_none());
	}

	#[test]
	fn dismissing_lasts_until_another_trigger_starts() {
		let mut state = typed("@ad");
		assert!(MENTION.dismiss(&mut state).expect("dismisses").is_some());
		assert!(MENTION.find(&state).is_none());
		assert_eq!(MENTION.query_value(&state), Value::Null);
		assert!(
			MENTION.dismiss(&mut state).expect("runs").is_none(),
			"nothing left to dismiss"
		);

		let other = Trigger {
			name: "other",
			..MENTION
		};
		assert!(
			other.find(&state).is_some(),
			"each name has its own setting"
		);

		commands::insert_text(&mut state, "a").expect("types");
		assert!(
			MENTION.find(&state).is_none(),
			"the same trigger stays dismissed"
		);

		commands::insert_text(&mut state, " @b").expect("types");
		assert_eq!(
			MENTION.find(&state).map(|found| found.query),
			Some("b".to_owned())
		);
	}

	#[test]
	fn the_query_value_is_a_record_of_the_trigger_and_query() {
		let value = MENTION.query_value(&typed("@ad"));
		let expected = Attrs::new().with("char", "@").with("query", "ad");

		assert_eq!(value, Value::from(expected));
		assert!(query_type().accepts(&value));
		assert!(query_type().accepts(&Value::Null));
	}

	#[test]
	fn replacing_adds_a_space_unless_one_follows() {
		let picked = || InlineText::plain("@Ada", MarkSet::empty());
		let mut state = typed("hi @ad");
		let found = MENTION.find(&state);
		replace(&mut state, found, picked()).expect("replaces");
		assert_eq!(state.doc().plain_text(), "hi @Ada ");
		assert_eq!(
			state.selection().as_caret().map(|caret| caret.offset),
			Some(8)
		);

		let mut state = typed("@ad x");
		let block = state.doc().children(None)[0];
		commands::set_selection(&mut state, Selection::caret(Point::new(block, 3))).expect("moves");
		let found = MENTION.find(&state);
		replace(&mut state, found, picked()).expect("replaces");
		assert_eq!(
			state.doc().plain_text(),
			"@Ada x",
			"the space after is reused"
		);
		assert_eq!(
			state.selection().as_caret().map(|caret| caret.offset),
			Some(5)
		);
	}

	#[test]
	fn replacing_without_a_suggestion_inserts_at_the_caret() {
		let mut state = typed("hi");
		replace(&mut state, None, InlineText::plain("!", MarkSet::empty())).expect("inserts");

		assert_eq!(state.doc().plain_text(), "hi! ");
	}
}
