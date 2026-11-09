#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_highlight`
//!
//! TipTap's `highlight` mark for the canvist editor, as in TipTap's
//! `@tiptap/extension-highlight`. [`Highlight`] registers the mark with the
//! `setHighlight`, `toggleHighlight`, and `unsetHighlight` commands and the
//! `isHighlight` query. `Mod-Shift-h` toggles a highlight in the theme's
//! colour, and typing or pasting `==text==` highlights the text.
//!
//! With [`Highlight::multicolor`] (the default, unlike TipTap) the mark has
//! a nullable `color` attribute, the set and toggle commands take an
//! optional `color`, and the `highlightColor` query reports the colour the
//! selection shares.
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_highlight::Highlight;
//! use canvist_extension_paragraph::Paragraph;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let highlight = builder.add(Highlight::default());
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! editor.run(&highlight.set(Some("#fef08a")), &mut env)?;
//! editor.insert_text("marked", &mut env)?;
//!
//! assert!(highlight.is_active(editor.state()));
//! assert_eq!(highlight.color(editor.state()).as_deref(), Some("#fef08a"));
//! # Ok::<(), canvist_core::EditorError>(())
//! ```

use canvist_core::Args;
use canvist_core::Binding;
use canvist_core::Command;
use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::Field;
use canvist_core::InputRule;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::PasteRule;
use canvist_core::Platform;
use canvist_core::QuerySpec;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::ValueType;
use canvist_core::command;
use canvist_core::commands;
use canvist_core::mark_type;
use canvist_core::query;
use canvist_model::Attrs;
use canvist_model::Mark;
use canvist_model::MarkSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Value;

/// Text between `==` pairs; group 1 is the text to highlight.
const PATTERN: &str = r"(?:^|\s)==([^=\s](?:[^=]*[^=\s])?)==";

/// A `highlight` mark, like a marker pen: `Mod-Shift-h` or `==text==`.
///
/// With `multicolor` (the default) the mark has a `color` attribute and
/// each highlight can have its own colour; without it, every highlight uses
/// the theme's colour. TipTap defaults to one colour; canvist defaults to
/// many because hosts that only use one simply never pass a colour.
#[derive(Clone, Copy, Debug)]
pub struct Highlight {
	/// Give each highlight a `color` attribute.
	pub multicolor: bool,
}

impl Default for Highlight {
	fn default() -> Self {
		Self { multicolor: true }
	}
}

/// Typed access to [`Highlight`].
#[derive(Clone, Copy, Debug)]
pub struct HighlightHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl HighlightHandle {
	/// Highlight the selection in `color`, or in the theme's colour for
	/// `None`. At a caret, the next typed text is highlighted.
	#[must_use]
	pub fn set(self, color: Option<&str>) -> Command {
		set(color)
	}

	/// Remove the highlight if the selection has one in `color`, otherwise
	/// highlight it in `color`.
	#[must_use]
	pub fn toggle(self, color: Option<&str>) -> Command {
		toggle(color)
	}

	/// Remove highlights from the selection.
	#[must_use]
	pub fn unset(self) -> Command {
		unset()
	}

	/// Whether the selection is highlighted.
	#[must_use]
	pub fn is_active(self, state: &EditorState) -> bool {
		query::mark_active(state, "highlight")
	}

	/// The colour every selected character is highlighted in, if they share
	/// one.
	#[must_use]
	pub fn color(self, state: &EditorState) -> Option<String> {
		color(state)
	}
}

/// A highlight in `color`. Without a colour (or without multicolour
/// highlights) the mark has no attributes, as typed `==text==` does, so the
/// two produce the same mark.
fn highlight(state: &EditorState, color: Option<&str>) -> Result<Mark, EditorError> {
	let kind = mark_type(state.schema(), "highlight", "highlight")?;
	let has_color = state.schema().mark(kind).attrs.get("color").is_some();

	Ok(match color {
		Some(color) if has_color => Mark::with_attrs(kind, Attrs::new().with("color", color)),
		_ => Mark::new(kind),
	})
}

fn set(color: Option<&str>) -> Command {
	let color = color.map(str::to_owned);
	command(move |state| {
		let mark = highlight(state, color.as_deref())?;
		commands::set_mark(state, &mark)
	})
}

fn toggle(color: Option<&str>) -> Command {
	let color = color.map(str::to_owned);
	command(move |state| {
		let mark = highlight(state, color.as_deref())?;
		let same = mark.attrs.get("color").and_then(Value::as_str) == self::color(state).as_deref();

		if commands::is_mark_active(state, mark.kind) && same {
			commands::unset_mark(state, mark.kind)
		} else {
			commands::set_mark(state, &mark)
		}
	})
}

fn unset() -> Command {
	command(|state| {
		let kind = mark_type(state.schema(), "highlight", "highlight")?;
		commands::unset_mark(state, kind)
	})
}

fn color(state: &EditorState) -> Option<String> {
	let value = query::shared_mark_attr(state, "highlight", "color")?;
	value.as_str().map(str::to_owned)
}

fn color_field() -> Field {
	Field::optional(
		"color",
		ValueType::nullable(ValueType::String),
		Value::Null,
		"The highlight colour, such as `#fef08a`, or null for the theme's colour.",
	)
}

/// The `color` argument, which only multicolour highlights declare.
fn color_arg<'a>(multicolor: bool, args: &Args<'a>) -> Result<Option<&'a str>, EditorError> {
	if multicolor {
		args.optional_string("color")
	} else {
		Ok(None)
	}
}

impl Highlight {
	/// The command arguments: a colour when highlights have one.
	fn args(self, spec: CommandSpec) -> CommandSpec {
		if self.multicolor {
			spec.arg(color_field())
		} else {
			spec
		}
	}
}

impl Extension for Highlight {
	fn name(&self) -> &'static str {
		"highlight"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		let attrs = if self.multicolor {
			Attrs::new().with("color", Value::Null)
		} else {
			Attrs::new()
		};

		schema.mark(MarkSpec::new("highlight").attrs(attrs))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		let node = NodeMeta::new(NodeKind::Mark, "highlight");

		if self.multicolor {
			vec![node.attr(color_field())]
		} else {
			vec![node]
		}
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let multicolor = self.multicolor;
		let set_spec = CommandSpec::new(
			"setHighlight",
			"Highlight the selection, in a colour or the theme's.",
			move |args| Ok(set(color_arg(multicolor, args)?)),
		);
		let toggle_spec = CommandSpec::new(
			"toggleHighlight",
			"Remove the selection's highlight in a colour, or highlight it in that colour.",
			move |args| Ok(toggle(color_arg(multicolor, args)?)),
		);

		Ok(vec![
			self.args(set_spec),
			self.args(toggle_spec),
			CommandSpec::simple(
				"unsetHighlight",
				"Remove highlights from the selection.",
				unset(),
			),
		])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		let active = QuerySpec::new(
			"isHighlight",
			"Whether the selection is highlighted.",
			ValueType::Bool,
			|state, _| Ok(Value::Bool(query::mark_active(state, "highlight"))),
		);

		if !self.multicolor {
			return Ok(vec![active]);
		}

		let color = QuerySpec::new(
			"highlightColor",
			"The colour every selected character is highlighted in, or null.",
			ValueType::nullable(ValueType::String),
			|state, _| Ok(color(state).map_or(Value::Null, Value::from)),
		);
		Ok(vec![active, color])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![("Mod-Shift-h", toggle(None))])
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let kind = mark_type(schema, "highlight", "highlight")?;
		Ok(vec![InputRule::mark(&format!("{PATTERN}$"), kind)?])
	}

	fn paste_rules(&self, schema: &Schema) -> Result<Vec<PasteRule>, EditorError> {
		let kind = mark_type(schema, "highlight", "highlight")?;
		Ok(vec![PasteRule::mark(PATTERN, kind)?])
	}
}

impl TypedExtension for Highlight {
	type Handle = HighlightHandle;

	fn handle(&self, registration: Registration) -> HighlightHandle {
		HighlightHandle {
			_registration: registration,
		}
	}
}

#[cfg(test)]
mod tests {
	use std::time::Duration;

	use canvist_core::CommandResult;
	use canvist_core::Editor;
	use canvist_core::Env;
	use canvist_extension_document::Document;
	use canvist_extension_paragraph::Paragraph;
	use canvist_model::Point;
	use canvist_model::Selection;

	use super::*;

	fn editor(highlight: Highlight) -> Result<(Editor, HighlightHandle), EditorError> {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		let handle = builder.add(highlight);
		Ok((builder.build()?, handle))
	}

	fn select(editor: &mut Editor, from: u32, to: u32, env: &mut Env<'_>) -> CommandResult {
		let block = editor.state().doc().children(None)[0];
		let selection = Selection::text(Point::new(block, from), Point::new(block, to));
		editor.select(selection, env)
	}

	#[test]
	fn typed_and_set_highlights_without_a_colour_match() -> Result<(), EditorError> {
		let (mut editor, highlight) = editor(Highlight::default())?;
		let mut env = Env::headless(Duration::ZERO);

		for character in "==a== b".chars() {
			editor.insert_text(&character.to_string(), &mut env)?;
		}

		select(&mut editor, 2, 3, &mut env)?;
		editor.run(&highlight.set(None), &mut env)?;

		let block = editor.state().doc().children(None)[0];
		let text = editor.state().doc().inline(block).expect("a text block");
		assert_eq!(text.as_str(), "a b");
		assert!(text.marks_after(0).is_some_and(|marks| !marks.is_empty()));
		assert_eq!(text.marks_after(0), text.marks_after(2));
		assert_eq!(highlight.color(editor.state()), None);

		editor.run(&highlight.toggle(None), &mut env)?;
		assert!(!highlight.is_active(editor.state()));
		Ok(())
	}

	#[test]
	fn toggle_removes_the_same_colour_and_recolours_another() -> Result<(), EditorError> {
		let (mut editor, highlight) = editor(Highlight::default())?;
		let mut env = Env::headless(Duration::ZERO);
		editor.insert_text("ab", &mut env)?;
		select(&mut editor, 0, 2, &mut env)?;

		editor.run(&highlight.toggle(Some("red")), &mut env)?;
		assert_eq!(highlight.color(editor.state()).as_deref(), Some("red"));
		editor.run(&highlight.toggle(Some("blue")), &mut env)?;
		assert_eq!(highlight.color(editor.state()).as_deref(), Some("blue"));
		editor.run(&highlight.toggle(Some("blue")), &mut env)?;
		assert!(!highlight.is_active(editor.state()));
		Ok(())
	}

	#[test]
	fn mixed_colours_share_no_colour() -> Result<(), EditorError> {
		let (mut editor, highlight) = editor(Highlight::default())?;
		let mut env = Env::headless(Duration::ZERO);
		editor.insert_text("ab", &mut env)?;
		select(&mut editor, 0, 1, &mut env)?;
		editor.run(&highlight.set(Some("red")), &mut env)?;
		select(&mut editor, 1, 2, &mut env)?;
		editor.run(&highlight.set(Some("blue")), &mut env)?;
		select(&mut editor, 0, 2, &mut env)?;

		assert!(highlight.is_active(editor.state()));
		assert_eq!(highlight.color(editor.state()), None);
		assert_eq!(editor.query("highlightColor", &Attrs::new())?, Value::Null);

		editor.run(&highlight.toggle(Some("red")), &mut env)?;
		assert_eq!(highlight.color(editor.state()).as_deref(), Some("red"));
		Ok(())
	}

	#[test]
	fn single_colour_highlights_ignore_colours() -> Result<(), EditorError> {
		let (mut editor, highlight) = editor(Highlight { multicolor: false })?;
		let mut env = Env::headless(Duration::ZERO);
		editor.insert_text("ab", &mut env)?;
		select(&mut editor, 0, 2, &mut env)?;

		editor.run(&highlight.set(Some("red")), &mut env)?;
		assert!(highlight.is_active(editor.state()));
		assert_eq!(highlight.color(editor.state()), None);
		assert!(editor.query("highlightColor", &Attrs::new()).is_err());

		editor.run(&highlight.toggle(Some("red")), &mut env)?;
		assert!(!highlight.is_active(editor.state()));
		Ok(())
	}
}
