#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_link`
//!
//! TipTap's `link` mark for the canvist editor. [`Link`] registers a
//! non-inclusive `link` mark with an `href` attribute, the `setLink` and
//! `unsetLink` commands, and the `linkHref` query. With `autolink`, a URL
//! becomes a link when a space follows it; with `link_on_paste`, URLs in
//! pasted text become links. Trailing punctuation such as a full stop stays
//! outside the link. Unlike TipTap, there is no `toggleLink` and no click
//! handling: the host decides what opening a link means.
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_link::Link;
//! use canvist_extension_paragraph::Paragraph;
//! use canvist_model::Point;
//! use canvist_model::Selection;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let link = builder.add(Link::default());
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! for character in "see https://canvist.dev ".chars() {
//! 	editor.insert_text(&character.to_string(), &mut env)?;
//! }
//!
//! let block = editor.state().doc().children(None)[0];
//! editor.select(Selection::caret(Point::new(block, 10)), &mut env)?;
//! assert_eq!(link.href(editor.state()).as_deref(), Some("https://canvist.dev"));
//! # Ok::<(), canvist_core::EditorError>(())
//! ```

use canvist_core::Command;
use canvist_core::CommandResult;
use canvist_core::CommandSpec;
use canvist_core::EditKind;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::Field;
use canvist_core::InputRule;
use canvist_core::Lookback;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::PasteRule;
use canvist_core::QuerySpec;
use canvist_core::Registration;
use canvist_core::RuleMatch;
use canvist_core::TypedExtension;
use canvist_core::ValueType;
use canvist_core::command;
use canvist_core::commands;
use canvist_core::mark_type;
use canvist_core::query;
use canvist_model::Attrs;
use canvist_model::Mark;
use canvist_model::MarkSpec;
use canvist_model::MarkTypeId;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Selection;
use canvist_model::Value;

/// A URL that does not swallow trailing punctuation.
const URL: &str = r#"https?://[^\s<>"']*[^\s<>"'.,;:!?)\]]"#;

/// A non-inclusive `link` mark with an `href`, so typing at a link's edge
/// does not extend it. URLs become links when pasted and when followed by
/// a space.
#[derive(Clone, Copy, Debug)]
pub struct Link {
	/// Link URLs typed into the text.
	pub autolink: bool,
	/// Link URLs in pasted text.
	pub link_on_paste: bool,
}

impl Default for Link {
	fn default() -> Self {
		Self {
			autolink: true,
			link_on_paste: true,
		}
	}
}

fn kind(schema: &Schema) -> Result<MarkTypeId, EditorError> {
	mark_type(schema, "link", "link")
}

fn link(kind: MarkTypeId, href: &str) -> Mark {
	Mark::with_attrs(kind, Attrs::new().with("href", href))
}

/// Typed access to [`Link`].
#[derive(Clone, Copy, Debug)]
pub struct LinkHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl LinkHandle {
	/// Link the selection to `href`, or update the link it already has. At a
	/// caret, the next typed text is linked.
	#[must_use]
	pub fn set(self, href: &str) -> Command {
		set(href)
	}

	/// Remove links from the selection.
	#[must_use]
	pub fn unset(self) -> Command {
		unset()
	}

	/// The `href` of the link at the selection, if every selected character
	/// shares one.
	#[must_use]
	pub fn href(self, state: &EditorState) -> Option<String> {
		href(state)
	}
}

fn set(href: &str) -> Command {
	let href = href.to_owned();
	command(move |state| {
		let kind = kind(state.schema())?;
		commands::set_mark(state, &link(kind, &href))
	})
}

fn unset() -> Command {
	command(|state| commands::unset_mark(state, kind(state.schema())?))
}

/// The link at the selection: the active marks at a caret, or the `href`
/// every selected character shares.
fn href(state: &EditorState) -> Option<String> {
	let href = query::shared_mark_attr(state, "link", "href")?;
	href.as_str().map(str::to_owned)
}

fn href_field(required: bool) -> Field {
	let doc = "The link target.";

	if required {
		Field::required("href", ValueType::String, doc)
	} else {
		Field::optional(
			"href",
			ValueType::nullable(ValueType::String),
			Value::Null,
			doc,
		)
	}
}

/// Link the URL before the caret (leaving trailing punctuation outside the
/// link), then type the space that ended it.
fn autolink(state: &mut EditorState, found: &RuleMatch, kind: MarkTypeId) -> CommandResult {
	let (Some(Some(url)), Some(href)) = (found.groups.get(1).cloned(), found.group(1)) else {
		return Ok(None);
	};
	let mark = link(kind, href);
	let caret = Point::new(found.block, found.caret);
	let marks = commands::marks_at(state.doc(), caret).without(kind);

	state.change(EditKind::InputRule, |tr| {
		tr.add_mark(found.block, url, mark)?;
		let end = tr.insert_text(caret, &found.typed, marks)?;

		tr.set_selection(Selection::caret(end));
		Ok(true)
	})
}

impl Extension for Link {
	fn name(&self) -> &'static str {
		"link"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		let attrs = Attrs::new().with("href", Value::Null);
		schema.mark(MarkSpec::new("link").inclusive(false).attrs(attrs))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Mark, "link").attr(href_field(false))]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let set_spec = CommandSpec::new(
			"setLink",
			"Link the selection to a URL, or update the link it already has.",
			|args| Ok(set(args.string("href")?)),
		);
		Ok(vec![
			set_spec.arg(href_field(true)),
			CommandSpec::simple("unsetLink", "Remove links from the selection.", unset()),
		])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		let query = QuerySpec::new(
			"linkHref",
			"The href of the link at the selection, or null.",
			ValueType::nullable(ValueType::String),
			|state, _| Ok(href(state).map_or(Value::Null, Value::from)),
		);
		Ok(vec![query])
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		if !self.autolink {
			return Ok(Vec::new());
		}

		let kind = kind(schema)?;
		// A URL may open with a bracket or quote, as in `(https://…)`.
		let pattern = format!(r#"(?:^|[\s(\[<"'])({URL})[.,;:!?)\]>"']*\s$"#);
		let rule = InputRule::new(&pattern, move |state, found| autolink(state, found, kind))?;
		Ok(vec![rule.lookback(Lookback::Word)])
	}

	fn paste_rules(&self, schema: &Schema) -> Result<Vec<PasteRule>, EditorError> {
		if !self.link_on_paste {
			return Ok(Vec::new());
		}

		let kind = kind(schema)?;
		Ok(vec![PasteRule::mark_with(URL, kind, |url| {
			Attrs::new().with("href", url)
		})?])
	}
}

impl TypedExtension for Link {
	type Handle = LinkHandle;

	fn handle(&self, registration: Registration) -> LinkHandle {
		LinkHandle {
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

	fn editor(link: Link) -> Result<(Editor, LinkHandle), EditorError> {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		let handle = builder.add(link);
		Ok((builder.build()?, handle))
	}

	fn type_text(editor: &mut Editor, text: &str, env: &mut Env<'_>) -> Result<(), EditorError> {
		for character in text.chars() {
			editor.insert_text(&character.to_string(), env)?;
		}

		Ok(())
	}

	/// Select the last `needle` in the paragraph and read its `href`.
	fn href_of(
		editor: &mut Editor,
		needle: &str,
		env: &mut Env<'_>,
	) -> Result<Option<String>, EditorError> {
		let block = editor.state().doc().children(None)[0];
		let text = editor.state().doc().plain_text();
		let start = text.rfind(needle).expect("the paragraph holds the needle");
		let point = |at: usize| Point::new(block, u32::try_from(at).expect("a short paragraph"));
		let selection = Selection::text(point(start), point(start + needle.len()));

		editor.select(selection, env)?;
		Ok(href(editor.state()))
	}

	#[test]
	fn mixed_links_share_no_href() -> Result<(), EditorError> {
		let (mut editor, link) = editor(Link::default())?;
		let mut env = Env::headless(Duration::ZERO);
		editor.insert_text("ab", &mut env)?;

		href_of(&mut editor, "a", &mut env)?;
		editor.run(&link.set("https://a.test"), &mut env)?;
		href_of(&mut editor, "b", &mut env)?;
		editor.run(&link.set("https://b.test"), &mut env)?;

		assert_eq!(href_of(&mut editor, "ab", &mut env)?, None);
		assert_eq!(
			href_of(&mut editor, "b", &mut env)?.as_deref(),
			Some("https://b.test")
		);
		Ok(())
	}

	#[test]
	fn autolink_skips_brackets_quotes_and_punctuation() -> Result<(), EditorError> {
		let cases = [
			("see https://a.test, ", ", "),
			("(https://a.test). ", "). "),
			("\"https://a.test\" ", "\" "),
		];

		for (typed, after) in cases {
			let (mut editor, _) = editor(Link::default())?;
			let mut env = Env::headless(Duration::ZERO);
			type_text(&mut editor, typed, &mut env)?;

			assert_eq!(editor.state().doc().plain_text(), typed);
			let url = href_of(&mut editor, "https://a.test", &mut env)?;
			assert_eq!(url.as_deref(), Some("https://a.test"), "{typed}");
			assert_eq!(href_of(&mut editor, after, &mut env)?, None, "{typed}");
		}

		Ok(())
	}

	#[test]
	fn options_turn_off_autolink_and_paste() -> Result<(), EditorError> {
		let options = Link {
			autolink: false,
			link_on_paste: false,
		};
		let (mut editor, _) = editor(options)?;
		let mut env = Env::headless(Duration::ZERO);
		type_text(&mut editor, "https://a.test ", &mut env)?;
		editor.paste_text("https://b.test", &mut env)?;

		assert_eq!(href_of(&mut editor, "https://a.test", &mut env)?, None);
		assert_eq!(href_of(&mut editor, "https://b.test", &mut env)?, None);
		Ok(())
	}

	#[test]
	fn pasted_urls_become_links() -> Result<(), EditorError> {
		let (mut editor, _) = editor(Link::default())?;
		let mut env = Env::headless(Duration::ZERO);
		editor.paste_text("go to https://a.test.", &mut env)?;

		let url = href_of(&mut editor, "https://a.test", &mut env)?;
		assert_eq!(url.as_deref(), Some("https://a.test"));
		assert_eq!(href_of(&mut editor, "go", &mut env)?, None);
		assert_eq!(href_of(&mut editor, ".", &mut env)?, None);
		Ok(())
	}

	#[test]
	fn unset_removes_the_link() -> Result<(), EditorError> {
		let (mut editor, link) = editor(Link::default())?;
		let mut env = Env::headless(Duration::ZERO);
		editor.insert_text("ab", &mut env)?;
		href_of(&mut editor, "ab", &mut env)?;

		editor.run(&link.set("https://a.test"), &mut env)?;
		editor.run(&link.unset(), &mut env)?;

		assert_eq!(link.href(editor.state()), None);
		Ok(())
	}
}
