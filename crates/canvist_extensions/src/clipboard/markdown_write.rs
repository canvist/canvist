//! Slices to Markdown (CommonMark with GitHub's strikethrough and task
//! lists).

use canvist_core::Slice;
use canvist_model::Attrs;
use canvist_model::BlockTree;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::Schema;
use canvist_model::TreeContent;
use canvist_model::Value;

use super::tree::HARD_BREAK;

/// Serialize a slice to Markdown. Highlights use `==`, and subscripts and
/// superscripts the inline `<sub>` and `<sup>` tags GitHub renders.
/// Underline and text styles have no Markdown form, so their text is
/// written plain; unknown blocks are written as their content.
#[must_use]
pub fn to_markdown(schema: &Schema, slice: &Slice) -> String {
	blocks(schema, slice.blocks(), "\n\n")
}

fn blocks(schema: &Schema, trees: &[BlockTree], separator: &str) -> String {
	rendered(schema, trees)
		.into_iter()
		.map(|(text, _)| text)
		.collect::<Vec<_>>()
		.join(separator)
}

/// The non-empty Markdown of each block. Adjacent lists of one family
/// alternate their marker (`-` and `*`, `.` and `)`), which is how
/// CommonMark keeps them apart instead of merging them into one list.
fn rendered<'t>(schema: &Schema, trees: &'t [BlockTree]) -> Vec<(String, &'t BlockTree)> {
	let mut out = Vec::new();
	let mut previous = None;

	for tree in trees {
		let marker =
			list_markers(schema.block(tree.kind).name.as_ref()).map(|(main, alternate)| {
				if previous == Some(main) {
					alternate
				} else {
					main
				}
			});
		let text = block(schema, tree, marker.unwrap_or('-'));

		if !text.is_empty() {
			previous = marker;
			out.push((text, tree));
		}
	}

	out
}

/// The default and alternate marker for a list type.
fn list_markers(name: &str) -> Option<(char, char)> {
	match name {
		"bulletList" | "taskList" => Some(('-', '*')),
		"orderedList" => Some(('.', ')')),
		_ => None,
	}
}

fn block(schema: &Schema, tree: &BlockTree, marker: char) -> String {
	let name = schema.block(tree.kind).name.as_ref();

	match &tree.content {
		TreeContent::Inline(text) => text_block(schema, name, &tree.attrs, text),
		TreeContent::Blocks(_) if cfg!(feature = "table") && (name == "table") => {
			table(schema, tree)
		}
		TreeContent::Blocks(children) => container(schema, name, &tree.attrs, children, marker),

		TreeContent::Leaf if name == "horizontalRule" => "---".to_owned(),
		TreeContent::Leaf if cfg!(feature = "ink") && (name == "ink") => {
			super::html_write::to_html(schema, &Slice::new(vec![tree.clone()]))
		}
		TreeContent::Leaf => media(name, &tree.attrs),
	}
}

/// A media block: an image as `![alt](src "title")`, and other media as a
/// link to their source. Unknown leaves write nothing.
fn media(name: &str, attrs: &Attrs) -> String {
	let text = |key: &str| attrs.get(key).and_then(Value::as_str).unwrap_or_default();

	match name {
		"image" if cfg!(feature = "image") => {
			let title = attrs
				.get("title")
				.and_then(Value::as_str)
				.map(|title| format!(" \"{title}\""))
				.unwrap_or_default();
			format!("![{}]({}{title})", escape(text("alt")), text("src"))
		}
		// A video that is not on its site was never embeddable; HTML drops
		// its source too.
		"youtube" | "twitch"
			if cfg!(any(feature = "youtube", feature = "twitch"))
				&& super::html_read::video_kind(text("src")) != Some(name) =>
		{
			String::new()
		}
		"youtube" | "twitch" | "audio"
			if cfg!(any(
				feature = "youtube",
				feature = "twitch",
				feature = "audio"
			)) =>
		{
			format!("[{0}]({0})", text("src"))
		}
		"file" if cfg!(feature = "file") => {
			// A link needs text to be read, and to be read back at all.
			let name = Some(text("fileName")).filter(|name| !name.is_empty());
			format!("[{}]({})", escape(name.unwrap_or(text("url"))), text("url"))
		}
		"blockMath" if cfg!(feature = "mathematics") => format!("$$\n{}\n$$", text("latex")),
		_ => String::new(),
	}
}

/// The deepest heading Markdown has.
const MAX_HEADING: i64 = 6;

/// Headings with their `#`s, code blocks fenced (with `~~~` when the code
/// itself holds a backtick fence), and paragraphs escaped line by line.
fn text_block(schema: &Schema, name: &str, attrs: &Attrs, text: &InlineText) -> String {
	match name {
		"heading" => {
			let level = attrs
				.get("level")
				.and_then(Value::as_int)
				.unwrap_or(1)
				.clamp(1, MAX_HEADING);
			let content = inline(schema, text);
			// A trailing `#` would read as the optional closing sequence.
			let content = match content.strip_suffix('#') {
				Some(head) => format!("{head}\\#"),
				None => content,
			};

			format!("{} {content}", "#".repeat(level as usize))
		}
		"codeBlock" => {
			let language = attrs
				.get("language")
				.and_then(Value::as_str)
				.unwrap_or_default();
			let code = super::tree::plain_text(schema, text);
			let fence = if code.contains("```") { "~~~" } else { "```" };
			format!("{fence}{language}\n{code}\n{fence}")
		}
		// Hard breaks start new lines, and each must not read as a block.
		_ => {
			inline(schema, text)
				.split('\n')
				.map(escape_line_start)
				.collect::<Vec<_>>()
				.join("\n")
		}
	}
}

fn container(
	schema: &Schema,
	name: &str,
	attrs: &Attrs,
	children: &[BlockTree],
	marker: char,
) -> String {
	match name {
		"blockquote" => prefix_lines(&blocks(schema, children, "\n\n"), "> ", ">"),
		"callout" if cfg!(feature = "callout") => {
			let alert = alert_of(attrs.get("type").and_then(Value::as_str).unwrap_or("info"));
			let content = blocks(schema, children, "\n\n");
			prefix_lines(&format!("[!{alert}]\n{content}"), "> ", ">")
		}
		"details" if cfg!(feature = "details") => details(schema, attrs, children),
		"bulletList" => list(schema, children, |_| format!("{marker} ")),
		"taskList" => {
			list(schema, children, |item| {
				let checked = item.attrs.get("checked").and_then(Value::as_bool) == Some(true);
				let box_ = if checked { "[x]" } else { "[ ]" };
				format!("{marker} {box_} ")
			})
		}
		"orderedList" => {
			let start = attrs.get("start").and_then(Value::as_int).unwrap_or(1);
			let mut number = start;
			list(schema, children, move |_| {
				let item_marker = format!("{number}{marker} ");
				number += 1;
				item_marker
			})
		}
		_ => blocks(schema, children, "\n\n"),
	}
}

/// A table as a GitHub pipe table when one can hold it exactly: a full grid
/// of single cells, each one line of text, with header cells in the first
/// row and nowhere else. Any other table is written as HTML, which GitHub
/// renders and the reader reads back.
fn table(schema: &Schema, tree: &BlockTree) -> String {
	pipe_table(schema, tree)
		.unwrap_or_else(|| super::html_write::to_html(schema, &Slice::new(vec![tree.clone()])))
}

fn pipe_table(schema: &Schema, tree: &BlockTree) -> Option<String> {
	let TreeContent::Blocks(rows) = &tree.content else {
		return None;
	};
	let mut lines = Vec::new();
	let mut columns = Vec::new();

	for (index, row) in rows.iter().enumerate() {
		let TreeContent::Blocks(cells) = &row.content else {
			return None;
		};

		if cells.is_empty() || cells.len() != table_width(rows) {
			return None;
		}

		// A pipe table aligns whole columns, as their header cells say.
		let aligns: Vec<Option<&str>> = cells.iter().map(cell_align).collect();

		if aligns.contains(&Some("justify")) || (index > 0 && aligns != columns) {
			return None;
		}

		if index == 0 {
			columns.clone_from(&aligns);
		}

		let texts = cells
			.iter()
			.map(|cell| pipe_cell(schema, cell, index == 0))
			.collect::<Option<Vec<_>>>()?;
		lines.push(format!("| {} |", texts.join(" | ")));

		if index == 0 {
			let delimiters: Vec<&str> = columns.iter().map(|align| delimiter(*align)).collect();
			lines.push(format!("| {} |", delimiters.join(" | ")));
		}
	}

	Some(lines.join("\n"))
}

/// How many cells the first row of a table has.
fn table_width(rows: &[BlockTree]) -> usize {
	match rows.first().map(|row| &row.content) {
		Some(TreeContent::Blocks(cells)) => cells.len(),
		_ => 0,
	}
}

/// A pipe table cell's text: a single cell of the expected kind holding one
/// line of text, with its pipes escaped.
fn pipe_cell(schema: &Schema, cell: &BlockTree, header: bool) -> Option<String> {
	let expected = if header { "tableHeader" } else { "tableCell" };
	let spans = ["colspan", "rowspan"].iter().any(|key| {
		cell.attrs
			.get(key)
			.and_then(Value::as_int)
			.is_some_and(|span| span > 1)
	});

	if schema.block(cell.kind).name.as_ref() != expected || spans {
		return None;
	}

	let TreeContent::Blocks(children) = &cell.content else {
		return None;
	};
	let [paragraph] = children.as_slice() else {
		return None;
	};
	let TreeContent::Inline(text) = &paragraph.content else {
		return None;
	};
	let markdown = inline(schema, text);

	(!markdown.contains('\n')).then(|| markdown.replace('|', "\\|"))
}

/// The alignment of a cell's paragraph.
fn cell_align(cell: &BlockTree) -> Option<&str> {
	match &cell.content {
		TreeContent::Blocks(children) => {
			children
				.first()
				.and_then(|paragraph| paragraph.attrs.get("textAlign"))
				.and_then(Value::as_str)
		}
		_ => None,
	}
}

/// The delimiter cell for a column aligned `align`.
fn delimiter(align: Option<&str>) -> &'static str {
	match align {
		Some("left") => ":---",
		Some("center") => ":---:",
		Some("right") => "---:",
		_ => "---",
	}
}

/// The GitHub alert for each callout type, and back.
pub(super) const ALERTS: [(&str, &str); 5] = [
	("info", "NOTE"),
	("success", "TIP"),
	("blank", "IMPORTANT"),
	("warning", "WARNING"),
	("error", "CAUTION"),
];

fn alert_of(kind: &str) -> &'static str {
	ALERTS
		.iter()
		.find(|(callout, _)| *callout == kind)
		.map_or("NOTE", |(_, alert)| alert)
}

/// Details as the `<details>` HTML GitHub renders, with the summary's text
/// and the content as Markdown between blank lines.
fn details(schema: &Schema, attrs: &Attrs, children: &[BlockTree]) -> String {
	let open = attrs.get("open").and_then(Value::as_bool) == Some(true);
	let summary = children
		.first()
		.and_then(|summary| {
			match &summary.content {
				TreeContent::Inline(text) => Some(super::tree::plain_text(schema, text)),
				_ => None,
			}
		})
		.unwrap_or_default();
	let content = children
		.get(1)
		.map(|content| block(schema, content, '-'))
		.unwrap_or_default();
	let tag = if open { "<details open>" } else { "<details>" };

	format!(
		"{tag}\n<summary>{}</summary>\n\n{content}\n\n</details>",
		super::html_write::escape(&summary)
	)
}

/// Write list items, each starting with its marker and with continuation
/// lines indented to line up under the item's text.
fn list(
	schema: &Schema,
	items: &[BlockTree],
	mut marker: impl FnMut(&BlockTree) -> String,
) -> String {
	let mut lines = Vec::new();

	for item in items {
		let marker = marker(item);
		// Continuation lines align with the item's content, which starts
		// before a task checkbox: the checkbox is part of the content.
		let width = marker.find('[').unwrap_or(marker.len());
		let indent = " ".repeat(width);

		let content = match &item.content {
			TreeContent::Blocks(children) => item_content(schema, children),
			TreeContent::Inline(_) | TreeContent::Leaf => block(schema, item, '-'),
		};

		let mut content_lines = content.lines();
		let first = content_lines.next().unwrap_or_default();
		lines.push(format!("{marker}{first}"));

		lines.extend(content_lines.map(|line| prefix_line(line, &indent, "")));
	}

	lines.join("\n")
}

/// An item's blocks: a nested list follows its paragraph on the next
/// line, while further paragraphs need a blank line between them.
fn item_content(schema: &Schema, children: &[BlockTree]) -> String {
	let mut content = String::new();

	for (index, (text, child)) in rendered(schema, children).into_iter().enumerate() {
		if index > 0 {
			let nested = matches!(child.content, TreeContent::Blocks(_));

			content.push_str(if nested { "\n" } else { "\n\n" });
		}

		content.push_str(&text);
	}

	content
}

/// Prefix every line, using `empty` for blank lines.
fn prefix_lines(text: &str, prefix: &str, empty: &str) -> String {
	text.lines()
		.map(|line| prefix_line(line, prefix, empty))
		.collect::<Vec<_>>()
		.join("\n")
}

fn prefix_line(line: &str, prefix: &str, empty: &str) -> String {
	if line.is_empty() {
		return empty.to_owned();
	}

	format!("{prefix}{line}")
}

/// Escape text that would otherwise open a heading, quote, list, fence,
/// or rule: the marker's first character, or an ordered list's delimiter.
fn escape_line_start(text: &str) -> String {
	if !super::markdown_read::starts_block(text) {
		return text.to_owned();
	}

	let indent = text.len() - text.trim_start_matches(' ').len();
	let digits = text[indent..]
		.chars()
		.take_while(char::is_ascii_digit)
		.count();
	let marker_end = indent + digits;

	format!("{}\\{}", &text[..marker_end], &text[marker_end..])
}

/// The inline HTML tags the Markdown reader understands, which literal text
/// must not spell out.
const INLINE_TAGS: [&str; 4] = ["<sub>", "</sub>", "<sup>", "</sup>"];

/// Escape characters with inline meaning: emphasis and code delimiters,
/// brackets, `==` (highlight), and `<` when it would open a tag the reader
/// understands.
fn escape(text: &str) -> String {
	escape_with(text, false)
}

/// [`escape`], also escaping `$` when `math` says it delimits formulas.
fn escape_with(text: &str, math: bool) -> String {
	let mut escaped = String::with_capacity(text.len());

	for (index, character) in text.char_indices() {
		let rest = &text[index..];
		let delimiter = matches!(character, '\\' | '*' | '_' | '`' | '[' | ']' | '~');
		let highlight = cfg!(feature = "highlight") && rest.starts_with("==");
		let tag = cfg!(any(feature = "subscript", feature = "superscript"))
			&& INLINE_TAGS.iter().any(|tag| rest.starts_with(tag));
		let dollar = math && character == '$';

		if delimiter || highlight || tag || dollar {
			escaped.push('\\');
		}

		escaped.push(character);
	}

	escaped
}

/// Whether Markdown writes `a` and `b` the same way: the same kind, and
/// for a link the same `href`. Any other attribute, such as a highlight's
/// colour, has no Markdown form, so marks that differ only in it share one
/// span: `==a b==`, not `==a====b==`, which reads back as one highlight
/// anyway.
fn same_in_markdown(schema: &Schema, a: &Mark, b: &Mark) -> bool {
	a.kind == b.kind && (a.attrs == b.attrs || schema.mark(a.kind).name.as_ref() != "link")
}

/// The delimiters around text with `mark`, or `None` for marks Markdown
/// cannot express.
fn delimiters(schema: &Schema, mark: &Mark) -> Option<(String, String)> {
	let pair = |delimiter: &str| Some((delimiter.to_owned(), delimiter.to_owned()));

	match schema.mark(mark.kind).name.as_ref() {
		"bold" => pair("**"),
		"italic" => pair("*"),
		"strike" => pair("~~"),
		"code" => pair("`"),
		"highlight" if cfg!(feature = "highlight") => pair("=="),
		"subscript" if cfg!(feature = "subscript") => {
			Some(("<sub>".to_owned(), "</sub>".to_owned()))
		}
		"superscript" if cfg!(feature = "superscript") => {
			Some(("<sup>".to_owned(), "</sup>".to_owned()))
		}
		"link" => {
			let href = mark
				.attrs
				.get("href")
				.and_then(Value::as_str)
				.unwrap_or_default();
			Some(("[".to_owned(), format!("]({href})")))
		}
		_ => None,
	}
}

/// A run's text split into leading whitespace, content, and trailing
/// whitespace, so whitespace can sit outside emphasis delimiters, where
/// Markdown requires it.
fn split_whitespace(text: &str) -> (&str, &str, &str) {
	let core_start = text.len() - text.trim_start().len();
	let core_end = text.trim_end().len().max(core_start);
	(
		&text[..core_start],
		&text[core_start..core_end],
		&text[core_end..],
	)
}

/// Builds inline Markdown run by run. Marks that continue across runs
/// stay open, marks closing out of order close (and reopen) the marks
/// opened after them so delimiters always nest, and whitespace at the edge
/// of formatted text moves outside the delimiters, where Markdown needs it.
struct InlineWriter<'s> {
	schema: &'s Schema,
	out: String,
	open: Vec<(Mark, String)>,
	/// Trailing whitespace held back until the marks around it close.
	pending: String,
	/// Where text sits in `out`, written as is: it is escaped in
	/// [`InlineWriter::finish`], once the characters around it are known.
	text: Vec<TextSpan>,
}

/// A stretch of `out` holding text, and whether `$` delimits formulas in it.
struct TextSpan {
	start: usize,
	end: usize,
	math: bool,
}

impl InlineWriter<'_> {
	/// Close every open mark that `wanted` does not keep, innermost first.
	fn close_unwanted(&mut self, wanted: &[&Mark]) {
		let keep = self
			.open
			.iter()
			.take_while(|(mark, _)| {
				wanted
					.iter()
					.any(|wanted| same_in_markdown(self.schema, mark, wanted))
			})
			.count();

		for (_, close) in self.open.drain(keep..).rev() {
			self.out.push_str(&close);
		}
	}

	fn in_code(&self) -> bool {
		self.open
			.iter()
			.any(|(mark, _)| self.schema.mark(mark.kind).name.as_ref() == "code")
	}

	/// Write one run's text with `marks`. `literal` text (hard breaks) is
	/// written as is and never sits inside a mark.
	fn write(&mut self, text: &str, marks: &[Mark], literal: bool) {
		let (lead, core, trail) = split_whitespace(text);

		if core.is_empty() {
			self.pending.push_str(lead);
			self.pending.push_str(trail);

			return;
		}

		let wanted: Vec<&Mark> = marks
			.iter()
			.filter(|mark| !literal && delimiters(self.schema, mark).is_some())
			.collect();
		self.close_unwanted(&wanted);
		self.out.push_str(&self.pending);
		self.out.push_str(lead);
		trail.clone_into(&mut self.pending);
		open_marks(self.schema, &wanted, core, &mut self.open, &mut self.out);

		if !literal && !self.in_code() {
			let math =
				cfg!(feature = "mathematics") && self.schema.embed_type("inlineMath").is_some();
			let start = self.out.len();
			self.text.push(TextSpan {
				start,
				end: start + core.len(),
				math,
			});
		}

		self.out.push_str(core);
	}

	fn finish(mut self) -> String {
		self.close_unwanted(&[]);
		self.out.push_str(&self.pending);
		escape_text(&self.out, &self.text)
	}
}

/// `out` with its text escaped where, and only where, a character would
/// otherwise read as syntax. Each decision sees the whole line, delimiters
/// included, so `snake_case`, `a * b`, `[a note]`, and the `_` in a bare
/// URL stay as written, while `*a*`, `[a](b)`, and `` `a` `` written as
/// text are escaped.
fn escape_text(out: &str, spans: &[TextSpan]) -> String {
	let urls = url_spans(out);
	let mut escaped = String::with_capacity(out.len() + out.len() / 8);
	let mut copied = 0;

	for span in spans {
		escaped.push_str(&out[copied..span.start]);

		for (offset, character) in out[span.start..span.end].char_indices() {
			let index = span.start + offset;
			let in_url = urls.iter().any(|url| url.contains(&index));

			if needs_escape(out, index, character, span.math, in_url) {
				escaped.push('\\');
			}

			escaped.push(character);
		}

		copied = span.end;
	}

	escaped.push_str(&out[copied..]);
	escaped
}

/// Whether the text character at `index` in `out` must be escaped to read
/// back as text, in CommonMark with GitHub's extensions and in canvist's
/// own reader alike.
fn needs_escape(out: &str, index: usize, character: char, math: bool, in_url: bool) -> bool {
	let rest = &out[index..];
	let before = out[..index].chars().next_back();
	let after = rest[character.len_utf8()..].chars().next();
	// The edges of a line count as whitespace, as in CommonMark's flanking rules.
	let space = |near: Option<char>| near.is_none_or(char::is_whitespace);
	let word = |near: Option<char>| near.is_some_and(char::is_alphanumeric);
	let highlight = cfg!(feature = "highlight") && rest.starts_with("==");
	let tag = cfg!(any(feature = "subscript", feature = "superscript"))
		&& INLINE_TAGS.iter().any(|tag| rest.starts_with(tag));

	if highlight || tag || (math && character == '$') {
		return true;
	}

	match character {
		// A backslash only escapes punctuation, and before a line end makes
		// a hard break; anywhere else it is a backslash.
		'\\' => after.is_none_or(|next| next.is_ascii_punctuation() || next == '\n'),
		// A delimiter with whitespace on both sides can neither open nor
		// close; `_` inside a word can't either, nor `_` and `~` in a URL.
		'*' => !(space(before) && space(after)),
		'_' => !(in_url || space(before) && space(after) || word(before) && word(after)),
		'~' => !(in_url || space(before) && space(after)),
		// A backtick opens a code span only if another closes it.
		'`' => {
			out.char_indices()
				.any(|(at, other)| other == '`' && at != index)
		}
		// Brackets only matter as a link, `[label](href)`, or at the start of
		// a line as a reference definition, `[label]: href`.
		'[' => rest.contains("](") || (index == 0 && rest.contains("]:")),
		']' => matches!(after, Some('(' | '[' | ':')),
		_ => false,
	}
}

/// Where `out` holds a bare URL, as GitHub's autolinks find them: `www.` or
/// an `http`/`https` URL after whitespace or the start of the line, up to
/// whitespace or `<`, without trailing punctuation.
pub(super) fn url_spans(out: &str) -> Vec<std::ops::Range<usize>> {
	let mut spans = Vec::new();

	for (index, _) in out.char_indices() {
		if spans
			.last()
			.is_some_and(|span: &std::ops::Range<usize>| span.contains(&index))
		{
			continue;
		}

		if let Some(length) = url_length(out, index) {
			spans.push(index..index + length);
		}
	}

	spans
}

/// The length of the bare URL starting at `index`, if one does.
pub(super) fn url_length(text: &str, index: usize) -> Option<usize> {
	let rest = &text[index..];
	let before = text[..index].chars().next_back();
	let starts = ["https://", "http://", "www."].iter().any(|scheme| {
		rest.len() > scheme.len()
			&& rest
				.get(..scheme.len())
				.is_some_and(|start| start.eq_ignore_ascii_case(scheme))
	});

	if !starts
		|| !before.is_none_or(|near| near.is_whitespace() || matches!(near, '(' | '*' | '_' | '~'))
	{
		return None;
	}

	let end = rest
		.find(|character: char| character.is_whitespace() || character == '<')
		.unwrap_or(rest.len());
	let mut url = &rest[..end];

	// Trailing punctuation, and a `)` with no `(` to match, end the URL.
	loop {
		// A backslash goes too, so an escaped `\_` at the end reads as the `_` it escapes.
		let trimmed =
			url.trim_end_matches(['?', '!', '.', ',', ':', '*', '_', '~', '\'', '"', '\\']);
		let unmatched =
			trimmed.ends_with(')') && trimmed.matches(')').count() > trimmed.matches('(').count();
		let next = if unmatched {
			&trimmed[..trimmed.len() - 1]
		} else {
			trimmed
		};

		if next.len() == url.len() {
			break;
		}

		url = next;
	}

	url.contains('.').then_some(url.len())
}

/// Inline text as Markdown. Markdown cannot break a line inside emphasis
/// or code, so a hard break closes every mark and the text after it
/// reopens them.
fn inline(schema: &Schema, text: &InlineText) -> String {
	let mut writer = InlineWriter {
		schema,
		out: String::new(),
		open: Vec::new(),
		pending: String::new(),
		text: Vec::new(),
	};

	for run in text.runs() {
		match run.embed {
			Some(embed) if schema.embed(embed.kind).name.as_ref() == HARD_BREAK => {
				writer.write("\\\n", &[], true);
			}

			Some(embed) => {
				let text = super::tree::embed_text(schema, embed);
				let literal = cfg!(feature = "mathematics")
					&& schema.embed(embed.kind).name.as_ref() == "inlineMath";
				writer.write(&text, run.marks.as_slice(), literal);
			}
			None => writer.write(run.text, run.marks.as_slice(), false),
		}
	}

	writer.finish()
}

/// Open the delimiters for every wanted mark that is not open yet. Code
/// delimiters are sized to `text`, so code containing backticks survives.
fn open_marks(
	schema: &Schema,
	wanted: &[&Mark],
	text: &str,
	open: &mut Vec<(Mark, String)>,
	out: &mut String,
) {
	for mark in wanted {
		if open
			.iter()
			.any(|(existing, _)| same_in_markdown(schema, existing, mark))
		{
			continue;
		}

		let delimiters = if schema.mark(mark.kind).name.as_ref() == "code" {
			Some(code_fence(text))
		} else {
			delimiters(schema, mark)
		};

		if let Some((start, close)) = delimiters {
			out.push_str(&start);
			open.push(((*mark).clone(), close));
		}
	}
}

/// Backticks longer than any run inside `code`, padded with a space when
/// the code itself starts or ends with a backtick.
fn code_fence(code: &str) -> (String, String) {
	let longest = code
		.split(|character| character != '`')
		.map(str::len)
		.max()
		.unwrap_or(0);
	let fence = "`".repeat(longest + 1);
	let pad = if code.starts_with('`') || code.ends_with('`') {
		" "
	} else {
		""
	};

	(format!("{fence}{pad}"), format!("{pad}{fence}"))
}
