//! Slices to HTML, in the markup TipTap renders.

use std::fmt::Write;

use canvist_core::Slice;
use canvist_model::Attrs;
use canvist_model::BlockTree;
use canvist_model::Embed;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::Schema;
use canvist_model::TreeContent;
use canvist_model::Value;
use canvist_model::is_safe_link_url;

use super::features::AUDIO;
use super::features::CALLOUT;
use super::features::COMMENTS;
use super::features::CONDITIONAL;
use super::features::DETAILS;
use super::features::FILE;
use super::features::HASHTAG;
use super::features::HIGHLIGHT;
use super::features::IMAGE;
use super::features::INK;
use super::features::KINETIC;
use super::features::MATHEMATICS;
use super::features::SUBSCRIPT;
use super::features::SUPERSCRIPT;
use super::features::TABLE;
use super::features::TEXT_STYLE;
use super::features::TWITCH;
use super::features::YOUTUBE;
use super::features::gated;
use super::tree::HARD_BREAK;

/// Serialize a slice to HTML: `<p>`, `<h1>`–`<h6>`, `<blockquote>`,
/// `<pre><code>`, `<ul>`, `<ol>`, `<li>`, `<hr>`, and inline `<strong>`,
/// `<em>`, `<u>`, `<s>`, `<code>`, `<a>`, `<sub>`, `<sup>`, `<mark>`,
/// `<span style>` for text styles, and `<br>`. Task lists use
/// TipTap's `data-type` and `data-checked` attributes. Unknown blocks are
/// written as `<div data-type>` around their content.
/// Links with unsupported URL schemes retain their text but omit `href`.
///
/// Styles accept simple alignment keywords, colours, font family lists, and
/// nonnegative sizes. Unsupported values (including CSS expressions, escapes,
/// and comments) are omitted so JSON and collaborative attributes cannot add
/// declarations or resource references. Text and other valid styles are kept.
#[must_use]
pub fn to_html(schema: &Schema, slice: &Slice) -> String {
	let mut html = String::new();

	for tree in slice.blocks() {
		write_block(schema, tree, &mut html);
	}

	html
}

fn attr_int(attrs: &Attrs, key: &str) -> Option<i64> {
	attrs.get(key).and_then(Value::as_int)
}

/// One block: a text block, a container around its children, or a leaf.
fn write_block(schema: &Schema, tree: &BlockTree, html: &mut String) {
	let name = schema.block(tree.kind).name.as_ref();

	match &tree.content {
		TreeContent::Inline(text) => write_text_block(schema, name, &tree.attrs, text, html),
		TreeContent::Blocks(children) => {
			let (open, close) = container_tags(name, &tree.attrs);
			html.push_str(&open);

			for child in children {
				write_block(schema, child, html);
			}

			html.push_str(close);
		}
		TreeContent::Leaf => write_leaf(name, &tree.attrs, html),
	}
}

/// A leaf block: a rule, a drawing, a media block, or an unknown leaf as
/// an empty `<div data-type>`.
fn write_leaf(name: &str, attrs: &Attrs, html: &mut String) {
	if let Some(media) = media(name) {
		write_media(name, media, attrs, html);

		return;
	}

	match gated(name) {
		"horizontalRule" => html.push_str("<hr>"),
		"ink" if INK => write_ink(attrs, html),
		_ => {
			let _ = write!(html, "<div data-type=\"{}\"></div>", escape(name));
		}
	}
}

/// How a media block's element opens and closes, and the HTML attribute
/// each of its attributes becomes.
struct Media {
	open: &'static str,
	attributes: &'static [(&'static str, &'static str)],
	close: &'static str,
}

/// The attributes a video frame carries.
const VIDEO_ATTRIBUTES: &[(&str, &str)] =
	&[("src", "src"), ("width", "width"), ("height", "height")];

/// Media blocks as TipTap and Remirror write them.
const MEDIA: [(&str, Media); 6] = [
	(
		"image",
		Media {
			open: "<img",
			attributes: &[
				("src", "src"),
				("alt", "alt"),
				("title", "title"),
				("width", "width"),
				("height", "height"),
			],
			close: ">",
		},
	),
	(
		"youtube",
		Media {
			open: "<div data-youtube-video=\"\"><iframe title=\"YouTube video\"",
			attributes: VIDEO_ATTRIBUTES,
			close: "></iframe></div>",
		},
	),
	(
		"twitch",
		Media {
			open: "<div data-twitch-video=\"\"><iframe title=\"Twitch video\"",
			attributes: VIDEO_ATTRIBUTES,
			close: "></iframe></div>",
		},
	),
	(
		"audio",
		Media {
			open: "<audio",
			attributes: &[("src", "src")],
			close: "></audio>",
		},
	),
	(
		"file",
		Media {
			open: "<div data-type=\"file\"",
			attributes: &[
				("url", "data-url"),
				("fileName", "data-file-name"),
				("fileType", "data-file-type"),
				("fileSize", "data-file-size"),
			],
			close: "></div>",
		},
	),
	(
		"blockMath",
		Media {
			open: "<div data-type=\"block-math\"",
			attributes: &[("latex", "data-latex")],
			close: "></div>",
		},
	),
];

/// The markup of media block `name`, when it is one and compiled in.
fn media(name: &str) -> Option<&'static Media> {
	// Constant per build, so the compiler drops media it never writes.
	let compiled = match name {
		"image" => IMAGE,
		"youtube" => YOUTUBE,
		"twitch" => TWITCH,
		"audio" => AUDIO,
		"file" => FILE,
		"blockMath" => MATHEMATICS,
		_ => false,
	};

	MEDIA
		.iter()
		.find(|(media, _)| compiled && *media == name)
		.map(|(_, media)| media)
}

/// A media block's element with the attributes it has set.
fn write_media(name: &str, media: &Media, attrs: &Attrs, html: &mut String) {
	html.push_str(media.open);

	for (key, attribute) in media.attributes {
		let Some(value) = media_value(attrs, key) else {
			continue;
		};

		// A video's source becomes a frame, so only its own site's URLs do.
		let is_video = matches!(name, "youtube" | "twitch");

		if is_video && *key == "src" && super::html_role::video_kind(&value) != Some(name) {
			continue;
		}

		write_attribute(html, attribute, &value);
	}

	if name == "audio" {
		write_audio_flags(attrs, html);
	}

	html.push_str(media.close);
}

/// A media attribute as HTML writes it: text, or a finite number.
fn media_value(attrs: &Attrs, key: &str) -> Option<String> {
	match attrs.get(key)? {
		Value::String(text) => Some(text.to_string()),
		Value::Int(number) => Some(number.to_string()),
		Value::Float(number) => float_text(*number),
		_ => None,
	}
}

/// `<audio>`'s boolean attributes. Controls are on unless turned off; the
/// rest are off.
fn write_audio_flags(attrs: &Attrs, html: &mut String) {
	for flag in ["controls", "autoplay", "loop", "muted"] {
		let on = attrs
			.get(flag)
			.and_then(Value::as_bool)
			.unwrap_or(flag == "controls");

		if on {
			html.push(' ');
			html.push_str(flag);
		}
	}
}

/// ` name="value"`, with the value escaped.
fn write_attribute(html: &mut String, name: &str, value: &str) {
	html.push(' ');
	html.push_str(name);
	html.push_str("=\"");
	html.push_str(&escape(value));
	html.push('"');
}

/// An ink drawing's height when it has none, as the ink extension's default.
const INK_HEIGHT: i64 = 400;

/// A stroke's width when it has none.
const STROKE_WIDTH: f64 = 4.0;

/// An ink drawing as a `<figure>` carrying its strokes as JSON, which the
/// reader takes back, around an SVG of them that any browser shows.
fn write_ink(attrs: &Attrs, html: &mut String) {
	// A malformed height would make the viewBox invalid.
	let height = attr_int(attrs, "height").unwrap_or(INK_HEIGHT).max(1);
	let strokes = attrs
		.get("strokes")
		.and_then(Value::as_array)
		.unwrap_or_default();
	let json = canvist_core::value_to_json(&Value::from(strokes.to_vec())).to_string();
	let _ = write!(
		html,
		"<figure data-type=\"ink\" data-height=\"{height}\" data-strokes=\"{}\" role=\"img\" \
		 aria-label=\"Drawing\"><svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1000 \
		 {height}\">",
		escape(&json)
	);

	for stroke in strokes.iter().filter_map(Value::as_map) {
		let points: Vec<f64> = stroke
			.get("points")
			.and_then(Value::as_array)
			.unwrap_or_default()
			.iter()
			.filter_map(|point| {
				match point {
					Value::Int(number) => Some(*number as f64),
					Value::Float(number) => Some(*number),
					_ => None,
				}
			})
			.collect();

		// A stroke without one whole point draws nothing.
		if points.len() < 2 {
			continue;
		}

		let color = stroke
			.get("color")
			.and_then(Value::as_str)
			.unwrap_or("currentColor");

		let width = match stroke.get("width") {
			Some(Value::Float(width)) => *width,
			Some(Value::Int(width)) => *width as f64,
			_ => STROKE_WIDTH,
		};

		let _ = write!(
			html,
			"<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" \
			 stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
			path_data(&points),
			escape(color),
			float_text(width).unwrap_or_default()
		);
	}

	html.push_str("</svg></figure>");
}

/// SVG path data through `x, y` pairs; one point is a zero-length segment.
fn path_data(points: &[f64]) -> String {
	let mut path = String::new();

	for (index, point) in points.chunks_exact(2).enumerate() {
		let command = if index == 0 { 'M' } else { 'L' };
		let (x, y) = (float_text(point[0]), float_text(point[1]));
		let _ = write!(
			path,
			"{command}{} {}",
			x.unwrap_or_default(),
			y.unwrap_or_default()
		);
	}

	if let [x, y] = points {
		let _ = write!(
			path,
			"L{} {}",
			float_text(*x).unwrap_or_default(),
			float_text(*y).unwrap_or_default()
		);
	}

	path
}

/// A finite number as HTML writes it: whole numbers without a fraction.
/// It goes through `serde_json`, whose float writer the bindings already
/// carry, rather than `f64`'s `Display`, which would add Rust's float
/// formatting to every WebAssembly build.
fn float_text(number: f64) -> Option<String> {
	const EXACT: f64 = 9_007_199_254_740_992.0;

	if number.fract() == 0.0 && number.abs() < EXACT {
		return Some((number as i64).to_string());
	}

	serde_json::Number::from_f64(number).map(|number| number.to_string())
}

/// A cell's opening tag, with its spans when they are more than one and
/// TipTap's comma-separated `colwidth`.
fn cell_tag(tag: &str, attrs: &Attrs) -> String {
	let mut open = format!("<{tag}");

	for key in ["colspan", "rowspan"] {
		if let Some(span) = attr_int(attrs, key).filter(|&span| span > 1) {
			let _ = write!(open, " {key}=\"{span}\"");
		}
	}

	let widths: Vec<String> = attrs
		.get("colwidth")
		.and_then(Value::as_array)
		.unwrap_or_default()
		.iter()
		.map(|width| width.as_int().unwrap_or(0).to_string())
		.collect();

	if widths.iter().any(|width| width != "0") {
		let _ = write!(open, " colwidth=\"{}\"", widths.join(","));
	}

	open.push('>');
	open
}

/// A container's opening and closing tags. Lists, cells, details, callouts,
/// and task items carry their attributes on the opening tag; an unknown
/// container is a `<div data-type>`.
fn container_tags(name: &str, attrs: &Attrs) -> (String, &'static str) {
	match gated(name) {
		"blockquote" => ("<blockquote>".to_owned(), "</blockquote>"),
		"bulletList" => ("<ul>".to_owned(), "</ul>"),
		"orderedList" => (ordered_list_tag(attrs), "</ol>"),
		"listItem" => ("<li>".to_owned(), "</li>"),
		"table" if TABLE => ("<table><tbody>".to_owned(), "</tbody></table>"),
		"tableRow" if TABLE => ("<tr>".to_owned(), "</tr>"),
		"tableCell" if TABLE => (cell_tag("td", attrs), "</td>"),
		"tableHeader" if TABLE => (cell_tag("th", attrs), "</th>"),
		"details" if DETAILS && flag(attrs, "open") => ("<details open>".to_owned(), "</details>"),
		"details" if DETAILS => ("<details>".to_owned(), "</details>"),
		"detailsContent" if DETAILS => ("<div data-type=\"detailsContent\">".to_owned(), "</div>"),
		"callout" if CALLOUT => {
			let kind = text_attr(attrs, "type").unwrap_or("info");
			(
				format!("<div data-callout-type=\"{}\">", escape(kind)),
				"</div>",
			)
		}
		"taskList" => ("<ul data-type=\"taskList\">".to_owned(), "</ul>"),
		"taskItem" => {
			let checked = flag(attrs, "checked");
			let open = format!("<li data-type=\"taskItem\" data-checked=\"{checked}\">");
			(open, "</li>")
		}
		_ => (format!("<div data-type=\"{}\">", escape(name)), "</div>"),
	}
}

/// `<ol>`, with its `start` when it is not one.
fn ordered_list_tag(attrs: &Attrs) -> String {
	match attr_int(attrs, "start").filter(|&start| start != 1) {
		Some(start) => format!("<ol start=\"{start}\">"),
		None => "<ol>".to_owned(),
	}
}

/// Whether boolean attribute `key` is set and true.
fn flag(attrs: &Attrs, key: &str) -> bool {
	attrs.get(key).and_then(Value::as_bool) == Some(true)
}

/// Text attribute `key`, if it holds text.
fn text_attr<'a>(attrs: &'a Attrs, key: &str) -> Option<&'a str> {
	attrs.get(key).and_then(Value::as_str)
}

/// The deepest heading HTML has.
const MAX_HEADING: i64 = 6;

/// A text block: a heading, a code block holding its plain text, a details
/// summary, or else a paragraph.
fn write_text_block(
	schema: &Schema,
	name: &str,
	attrs: &Attrs,
	text: &InlineText,
	html: &mut String,
) {
	match gated(name) {
		"heading" => {
			let level = attr_int(attrs, "level").unwrap_or(1).clamp(1, MAX_HEADING);
			let _ = write!(html, "<h{level}");
			write_align(attrs, html);
			html.push('>');
			write_inline(schema, text, html);
			let _ = write!(html, "</h{level}>");
		}
		"codeBlock" => {
			match text_attr(attrs, "language") {
				Some(language) => {
					let _ = write!(html, "<pre><code class=\"language-{}\">", escape(language));
				}
				None => html.push_str("<pre><code>"),
			}

			html.push_str(&escape(&super::tree::plain_text(schema, text)));
			html.push_str("</code></pre>");
		}
		"detailsSummary" if DETAILS => {
			html.push_str("<summary>");
			write_inline(schema, text, html);
			html.push_str("</summary>");
		}
		_ => {
			html.push_str("<p");
			write_align(attrs, html);
			html.push('>');
			write_inline(schema, text, html);
			html.push_str("</p>");
		}
	}
}

/// A ` style="text-align: …"` attribute for an aligned block, as TipTap
/// writes it, or nothing, as it is without the text alignment extension.
/// Every paragraph passes through here, so it writes into `html` rather
/// than allocating.
fn write_align(attrs: &Attrs, html: &mut String) {
	if !cfg!(feature = "text-align") {
		return;
	}

	let align =
		text_attr(attrs, "textAlign").and_then(|value| super::css::value("text-align", value));

	if let Some(align) = align {
		let _ = write!(html, " style=\"text-align: {}\"", escape(align));
	}
}

/// The opening and closing tags for one mark. Marks with attributes write
/// them as TipTap does; an unknown mark is a `<span data-mark>`.
fn mark_tags(schema: &Schema, mark: &Mark) -> (String, &'static str) {
	let name = schema.mark(mark.kind).name.as_ref();
	let attrs = &mark.attrs;

	match gated(name) {
		"bold" => ("<strong>".to_owned(), "</strong>"),
		"italic" => ("<em>".to_owned(), "</em>"),
		"underline" => ("<u>".to_owned(), "</u>"),
		"strike" => ("<s>".to_owned(), "</s>"),
		"code" => ("<code>".to_owned(), "</code>"),
		"link" => (link_tag(attrs), "</a>"),
		"subscript" if SUBSCRIPT => ("<sub>".to_owned(), "</sub>"),
		"superscript" if SUPERSCRIPT => ("<sup>".to_owned(), "</sup>"),
		"highlight" if HIGHLIGHT => (highlight_tag(attrs), "</mark>"),
		"textStyle" if TEXT_STYLE => (text_style_tag(attrs), "</span>"),
		"kinetic" if KINETIC => {
			let effect = text_attr(attrs, "effect").unwrap_or("wave");
			(span_tag(&[("data-kinetic", effect)]), "</span>")
		}
		"comment" if COMMENTS => {
			let threads: Vec<&str> = attrs
				.get("threads")
				.and_then(Value::as_array)
				.unwrap_or_default()
				.iter()
				.filter_map(Value::as_str)
				.collect();
			let threads = threads.join(" ");
			let tag = span_tag(&[("data-type", "comment"), ("data-threads", &threads)]);
			(tag, "</span>")
		}
		"conditional" if CONDITIONAL => {
			let when = text_attr(attrs, "when").unwrap_or_default();
			let tag = span_tag(&[("data-type", "conditional"), ("data-when", when)]);
			(tag, "</span>")
		}
		"hashtag" if HASHTAG => {
			let tag = match text_attr(attrs, "tag") {
				Some(tag) => span_tag(&[("data-type", "hashtag"), ("data-tag", tag)]),
				None => span_tag(&[("data-type", "hashtag")]),
			};

			(tag, "</span>")
		}
		_ => (span_tag(&[("data-mark", name)]), "</span>"),
	}
}

/// A `<span>` opening tag with these attributes.
fn span_tag(attributes: &[(&str, &str)]) -> String {
	let mut tag = "<span".to_owned();

	for (name, value) in attributes {
		write_attribute(&mut tag, name, value);
	}

	tag.push('>');
	tag
}

/// `<a>`, with its `href` when the URL's scheme is safe to follow.
fn link_tag(attrs: &Attrs) -> String {
	let href = text_attr(attrs, "href").unwrap_or_default();

	if is_safe_link_url(href) {
		format!("<a href=\"{}\">", escape(href))
	} else {
		"<a>".to_owned()
	}
}

/// `<mark>`, with TipTap's `data-color` and inline background when the
/// highlight has a colour.
fn highlight_tag(attrs: &Attrs) -> String {
	match attrs
		.get("color")
		.and_then(Value::as_str)
		.and_then(|value| super::css::value("background-color", value))
	{
		Some(color) => {
			let color = escape(color);
			format!(
				"<mark data-color=\"{color}\" style=\"background-color: {color}; color: inherit\">"
			)
		}
		None => "<mark>".to_owned(),
	}
}

/// The CSS properties of `textStyle` attributes, in the order TipTap writes
/// them.
pub(super) const TEXT_STYLE_PROPERTIES: [(&str, &str); 5] = [
	("color", "color"),
	("backgroundColor", "background-color"),
	("fontFamily", "font-family"),
	("fontSize", "font-size"),
	("lineHeight", "line-height"),
];

/// `<span>` with a `style` declaration per set `textStyle` attribute.
fn text_style_tag(attrs: &Attrs) -> String {
	let declarations: Vec<String> = TEXT_STYLE_PROPERTIES
		.iter()
		.filter_map(|(attr, property)| {
			let value = super::css::value(property, attrs.get(attr)?.as_str()?)?;
			Some(format!("{property}: {value}"))
		})
		.collect();

	if declarations.is_empty() {
		return "<span>".to_owned();
	}

	format!("<span style=\"{}\">", escape(&declarations.join("; ")))
}

/// Inline text, run by run: each run's marks open in order around its text,
/// a hard break, or an embed, and close in reverse, so tags always nest.
fn write_inline(schema: &Schema, text: &InlineText, html: &mut String) {
	for run in text.runs() {
		let tags: Vec<_> = run
			.marks
			.as_slice()
			.iter()
			.map(|mark| mark_tags(schema, mark))
			.collect();

		for (open, _) in &tags {
			html.push_str(open);
		}

		match run.embed {
			Some(embed) if schema.embed(embed.kind).name.as_ref() == HARD_BREAK => {
				html.push_str("<br>");
			}

			Some(embed) => write_embed(schema, embed, html),
			None => html.push_str(&escape(run.text)),
		}

		for (_, close) in tags.iter().rev() {
			html.push_str(close);
		}
	}
}

/// The `data-` attributes TipTap writes for each embed attribute.
const EMBED_ATTRIBUTES: [(&str, &str); 6] = [
	("id", "data-id"),
	("label", "data-label"),
	("mentionSuggestionChar", "data-mention-suggestion-char"),
	("name", "data-name"),
	("emoji", "data-emoji"),
	("latex", "data-latex"),
];

/// An inline embed as TipTap writes it, a `<span data-type>` with its
/// attributes as `data-` attributes and its text inside, such as
/// `<span data-type="mention" data-id="7" data-label="Ada">@Ada</span>`.
fn write_embed(schema: &Schema, embed: &Embed, html: &mut String) {
	let name = &*schema.embed(embed.kind).name;
	// TipTap's math node writes its type with a hyphen.
	let data_type = if name == "inlineMath" {
		"inline-math"
	} else {
		name
	};

	html.push_str("<span data-type=\"");
	html.push_str(&escape(data_type));
	html.push('"');

	for (attr, data) in EMBED_ATTRIBUTES {
		if let Some(value) = text_attr(&embed.attrs, attr) {
			write_attribute(html, data, value);
		}
	}

	html.push('>');
	html.push_str(&escape(&super::tree::embed_text(schema, embed)));
	html.push_str("</span>");
}

/// Escape text for HTML content and attribute values.
pub(super) fn escape(text: &str) -> String {
	let mut escaped = String::with_capacity(text.len());

	for character in text.chars() {
		match character {
			'&' => escaped.push_str("&amp;"),
			'<' => escaped.push_str("&lt;"),
			'>' => escaped.push_str("&gt;"),
			'"' => escaped.push_str("&quot;"),
			other => escaped.push(other),
		}
	}

	escaped
}
