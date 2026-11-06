//! Markdown reveal: the Markdown syntax of the formatting at the caret,
//! shown around it as you edit, as Typora and Obsidian do.
//!
//! Formatting stays formatting: the `**` around bold text is an
//! [`Adornment`], drawn faint while the caret touches the bold text and
//! gone when it leaves, so the document, undo, collaboration, and export
//! never see it. Each piece of syntax sits between the two carets at its
//! offset, and editing it edits the formatting:
//!
//! - The arrow keys stop on both sides of it. Typing just inside `**` is
//!   bold; typing just outside it is not.
//! - Backspace or Delete over it deletes one of its characters: `**` becomes
//!   `*`, so bold turns italic; `*`, `~~`, `` ` ``, and a link's `[` remove
//!   their formatting; and a heading's `##` becomes `#`, then a paragraph.

use std::sync::Arc;

use canvist_core::Adornment;
use canvist_core::Affinity;
use canvist_core::Binding;
use canvist_core::Command;
use canvist_core::CommandResult;
use canvist_core::EditKind;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::Platform;
use canvist_core::Update;
use canvist_core::command;
use canvist_core::text;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::Document;
use canvist_model::Mark;
use canvist_model::MarkSet;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::Selection;
use canvist_model::Value;

const MARKDOWN_REVEAL: &str = "markdownReveal";

/// Shows the Markdown syntax of the formatting at the caret, and lets you
/// edit it: see the [module documentation](self). Register it with the
/// formatting it reveals: bold, italic, strike, code, links, and headings.
#[derive(Clone, Copy, Debug, Default)]
pub struct MarkdownReveal;

impl Extension for MarkdownReveal {
	fn name(&self) -> &'static str {
		MARKDOWN_REVEAL
	}

	/// Above the standard keys and the formatting extensions, so the arrow
	/// keys, Backspace, and Delete reach the syntax first.
	fn priority(&self) -> i32 {
		150
	}

	fn adorns(&self) -> bool {
		true
	}

	fn adornments(&self, state: &EditorState) -> Vec<Adornment> {
		caret(state)
			.map(|(point, _)| reveal(state.doc(), point).adornments())
			.unwrap_or_default()
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![
			("ArrowRight", step(Side::After)),
			("ArrowLeft", step(Side::Before)),
			("Backspace", erase(Side::Before)),
			("Delete", erase(Side::After)),
		])
	}

	/// Typing at the syntax follows the side of it the caret is on, so
	/// moving the caret there chooses the formatting of what comes next.
	fn after_update(&self, state: &mut EditorState, update: &Update) -> CommandResult {
		// Only moves and typing choose the side; formatting at a caret, as
		// `Mod-b` does, sets the marks typing gets on purpose. Moving across
		// the syntax sets them itself, in `step`.
		if !update.selection_changed() && update.kind != EditKind::Typing {
			return Ok(None);
		}

		let Some((point, affinity)) = caret(state) else {
			return Ok(None);
		};

		if !reveal(state.doc(), point).has_syntax_at(point.offset) {
			return Ok(None);
		}

		// After typing, the caret stays with the text it typed: typing just
		// outside `**bold**` goes on outside it.
		let affinity = if update.kind == EditKind::Typing {
			Affinity::Upstream
		} else {
			affinity
		};
		let marks = typing_marks(state.doc(), point, affinity);

		if state.stored_marks() == Some(&marks) && state.affinity() == affinity {
			return Ok(None);
		}

		// Every change resets the affinity unless it says otherwise, and the
		// affinity is which side of the syntax the caret is on.
		state.change(EditKind::Selection, |tr| {
			tr.set_stored_marks(Some(marks));
			tr.set_affinity(affinity);
			Ok(true)
		})
	}
}

impl canvist_core::TypedExtension for MarkdownReveal {
	type Handle = canvist_core::Registration;

	fn handle(&self, registration: canvist_core::Registration) -> Self::Handle {
		registration
	}
}

/// Which side of the caret a key acts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
	Before,
	After,
}

/// A formatted span the caret touches, and the Markdown around it.
#[derive(Clone, Debug)]
struct Span {
	mark: Mark,
	name: Arc<str>,
	start: u32,
	end: u32,
}

impl Span {
	fn opening(&self) -> String {
		match self.name.as_ref() {
			"link" => "[".to_owned(),
			name => delimiter(name).unwrap_or_default().to_owned(),
		}
	}

	fn closing(&self) -> String {
		match self.name.as_ref() {
			"link" => {
				let href = self
					.mark
					.attrs
					.get("href")
					.and_then(Value::as_str)
					.unwrap_or_default();
				format!("]({href})")
			}
			name => delimiter(name).unwrap_or_default().to_owned(),
		}
	}
}

/// The Markdown delimiter of a mark this extension reveals.
fn delimiter(name: &str) -> Option<&'static str> {
	match name {
		"bold" => Some("**"),
		"italic" => Some("*"),
		"strike" => Some("~~"),
		"code" => Some("`"),
		"link" => Some("["),
		_ => None,
	}
}

/// What shows around a caret: the formatted spans it touches, and its
/// block's heading level.
#[derive(Clone, Debug)]
struct Reveal {
	block: BlockId,
	spans: Vec<Span>,
	heading: Option<i64>,
}

impl Reveal {
	fn adornments(&self) -> Vec<Adornment> {
		// (offset, order, text): a heading's `#`s come first, then closing
		// syntax, innermost first, then opening syntax, outermost first.
		let mut pieces: Vec<(u32, (u8, u32, u32), String)> = Vec::new();

		if let Some(level) = self.heading {
			let level = usize::try_from(level).unwrap_or(1);
			pieces.push((0, (0, 0, 0), format!("{} ", "#".repeat(level))));
		}

		for span in &self.spans {
			pieces.push((span.end, (1, u32::MAX - span.start, 0), span.closing()));
			pieces.push((span.start, (2, u32::MAX - span.end, 0), span.opening()));
		}

		pieces.sort_by_key(|(offset, order, _)| (*offset, *order));
		pieces
			.into_iter()
			.map(|(offset, _, text)| Adornment::new(Point::new(self.block, offset), &text))
			.collect()
	}

	/// Whether syntax shows at `offset`.
	fn has_syntax_at(&self, offset: u32) -> bool {
		(offset == 0 && self.heading.is_some())
			|| self
				.spans
				.iter()
				.any(|span| span.start == offset || span.end == offset)
	}
}

/// The collapsed caret, if the selection is one, and its affinity.
fn caret(state: &EditorState) -> Option<(Point, Affinity)> {
	state
		.selection()
		.as_caret()
		.map(|point| (point, state.affinity()))
}

/// What shows around a caret at `point`.
fn reveal(doc: &Document, point: Point) -> Reveal {
	let schema = doc.schema();
	let heading = doc.block(point.block).and_then(|block| {
		(schema.block(block.kind()).name.as_ref() == "heading").then(|| {
			block
				.attrs()
				.get("level")
				.and_then(Value::as_int)
				.unwrap_or(1)
		})
	});
	let spans = doc
		.inline(point.block)
		.map(|inline| spans(schema, inline, point.offset))
		.unwrap_or_default();

	Reveal {
		block: point.block,
		spans,
		heading,
	}
}

/// The revealed formatting touching `offset`: every mark on the text just
/// before or just after it, each with the whole run of text it covers.
fn spans(schema: &Schema, inline: &canvist_model::InlineText, offset: u32) -> Vec<Span> {
	let runs: Vec<(u32, u32, &MarkSet)> = inline
		.runs()
		.map(|run| (run.range.start, run.range.end, run.marks))
		.collect();

	let touching = runs
		.iter()
		.filter(|(start, end, _)| {
			*start < offset && offset <= *end || *start <= offset && offset < *end
		})
		.flat_map(|(_, _, marks)| marks.as_slice().iter());
	let mut spans: Vec<Span> = Vec::new();

	for mark in touching {
		let name = schema.mark(mark.kind).name.clone();

		if delimiter(&name).is_none() || spans.iter().any(|span| &span.mark == mark) {
			continue;
		}

		let covers = |marks: &MarkSet| marks.get(mark.kind) == Some(mark);
		let Some(first) = runs
			.iter()
			.position(|(start, end, marks)| covers(marks) && *start <= offset && offset <= *end)
		else {
			continue;
		};
		let start_index = runs[..=first]
			.iter()
			.rposition(|(_, _, marks)| !covers(marks))
			.map_or(0, |index| index + 1);
		let end_index = runs[first..]
			.iter()
			.position(|(_, _, marks)| !covers(marks))
			.map_or(runs.len(), |index| first + index);

		spans.push(Span {
			mark: mark.clone(),
			name,
			start: runs[start_index].0,
			end: runs[end_index - 1].1,
		});
	}

	spans
}

/// The marks typing at a caret on revealed syntax gets: those of the text
/// on the caret's side of it.
fn typing_marks(doc: &Document, point: Point, affinity: Affinity) -> MarkSet {
	let Some(inline) = doc.inline(point.block) else {
		return MarkSet::empty();
	};
	let marks = match affinity {
		Affinity::Upstream => inline.marks_before(point.offset),
		Affinity::Downstream => inline.marks_after(point.offset),
	};

	marks.cloned().unwrap_or_default()
}

/// Move the caret one step towards `side`, stopping on both sides of
/// revealed syntax. Other moves are left to the standard keys.
fn step(side: Side) -> Command {
	command(move |state| {
		let Some((point, affinity)) = caret(state) else {
			return Ok(None);
		};
		let doc = state.doc();

		// Across the syntax at the caret, without moving in the text.
		let across = match side {
			Side::After => affinity == Affinity::Upstream,
			Side::Before => affinity == Affinity::Downstream,
		};

		if across && reveal(doc, point).has_syntax_at(point.offset) {
			let affinity = match side {
				Side::After => Affinity::Downstream,
				Side::Before => Affinity::Upstream,
			};

			return select(state, point, affinity);
		}

		// Onto syntax: stop on the near side of it first.
		let Some(content) = doc
			.inline(point.block)
			.map(canvist_model::InlineText::as_str)
		else {
			return Ok(None);
		};
		let next = match side {
			Side::After => text::next_grapheme(content, point.offset),
			Side::Before => text::prev_grapheme(content, point.offset),
		};
		let Some(next) = next else {
			return Ok(None);
		};
		let target = Point::new(point.block, next);

		if !reveal(doc, target).has_syntax_at(next) {
			return Ok(None);
		}

		let affinity = match side {
			Side::After => Affinity::Upstream,
			Side::Before => Affinity::Downstream,
		};
		select(state, target, affinity)
	})
}

/// A caret at `point` on the `affinity` side of the syntax there, typing
/// with the marks of that side.
fn select(state: &mut EditorState, point: Point, affinity: Affinity) -> CommandResult {
	let marks = typing_marks(state.doc(), point, affinity);

	state.change(EditKind::Selection, |tr| {
		tr.set_selection(Selection::Text {
			anchor: point,
			head: point,
		});
		tr.set_affinity(affinity);
		tr.set_stored_marks(Some(marks));
		Ok(true)
	})
}

/// Delete one character of the syntax on `side` of the caret, when the
/// caret is on the far side of it from the text it formats: see the
/// [module documentation](self).
fn erase(side: Side) -> Command {
	command(move |state| {
		let Some((point, affinity)) = caret(state) else {
			return Ok(None);
		};
		let shown = reveal(state.doc(), point);
		let offset = point.offset;

		// A heading's `#`s, from just after them or just before them.
		let at_hashes = offset == 0
			&& matches!(
				(side, affinity),
				(Side::Before, Affinity::Downstream) | (Side::After, Affinity::Upstream)
			);

		if at_hashes && let Some(level) = shown.heading {
			return lower_heading(state, shown.block, level, affinity);
		}

		// The syntax the key would delete a character of: Backspace takes
		// from the opening syntax the caret is just inside (innermost first)
		// or the closing syntax it is just after (outermost first); Delete
		// mirrors it.
		let span = match (side, affinity) {
			(Side::Before, Affinity::Downstream) => {
				innermost(&shown, |span| span.start == offset)
					.or_else(|| outermost(&shown, |span| span.end == offset))
			}
			(Side::After, Affinity::Upstream) => {
				innermost(&shown, |span| span.end == offset)
					.or_else(|| outermost(&shown, |span| span.start == offset))
			}
			_ => None,
		};
		let Some(span) = span.cloned() else {
			return Ok(None);
		};

		unformat(state, shown.block, &span, affinity)
	})
}

/// The span `matches` picks with the least text.
fn innermost(shown: &Reveal, matches: impl Fn(&Span) -> bool) -> Option<&Span> {
	shown
		.spans
		.iter()
		.filter(|span| matches(span))
		.min_by_key(|span| span.end - span.start)
}

/// The span `matches` picks with the most text.
fn outermost(shown: &Reveal, matches: impl Fn(&Span) -> bool) -> Option<&Span> {
	shown
		.spans
		.iter()
		.filter(|span| matches(span))
		.max_by_key(|span| span.end - span.start)
}

/// One character less of `span`'s syntax: bold's `**` becomes italic's
/// `*`, and anything else goes.
fn unformat(
	state: &mut EditorState,
	block: BlockId,
	span: &Span,
	affinity: Affinity,
) -> CommandResult {
	let schema = state.doc().schema().clone();
	let italic = schema.mark_type("italic");
	let already_italic = italic.is_some_and(|italic| {
		state.doc().inline(block).is_some_and(|inline| {
			inline
				.mark_runs(span.start..span.end)
				.is_ok_and(|runs| runs.iter().all(|(_, marks)| marks.contains(italic)))
		})
	});
	let replace_with = (span.name.as_ref() == "bold" && !already_italic)
		.then_some(italic)
		.flatten();

	state.change(EditKind::Formatting, |tr| {
		tr.remove_mark(block, span.start..span.end, span.mark.kind)?;

		if let Some(italic) = replace_with {
			tr.add_mark(block, span.start..span.end, Mark::new(italic))?;
		}

		// The caret stays on its side of what syntax is left.
		tr.set_affinity(affinity);
		Ok(true)
	})
}

/// A heading one level up, or a paragraph after level one.
fn lower_heading(
	state: &mut EditorState,
	block: BlockId,
	level: i64,
	affinity: Affinity,
) -> CommandResult {
	let schema = state.doc().schema().clone();
	let (kind, attrs) = if level > 1 {
		let Some(heading) = schema.block_type("heading") else {
			return Ok(None);
		};
		(heading, Attrs::new().with("level", level - 1))
	} else {
		let Some(paragraph) = schema.block_type("paragraph") else {
			return Ok(None);
		};
		(paragraph, Attrs::new())
	};

	state.change(EditKind::Structure, |tr| {
		tr.set_block(block, kind, attrs)?;
		tr.set_affinity(affinity);
		Ok(true)
	})
}
