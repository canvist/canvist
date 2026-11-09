//! What each HTML element means to the reader: the block, mark, or embed
//! it stands for, and the attributes it carries.

use canvist_model::Attrs;
use canvist_model::Value;

use super::features::AUDIO;
use super::features::CALLOUT;
use super::features::DETAILS;
use super::features::EMOJI;
use super::features::FILE;
use super::features::IMAGE;
use super::features::INK;
use super::features::MATHEMATICS;
use super::features::MENTION;
use super::features::TABLE;
use super::features::TWITCH;
use super::features::YOUTUBE;
use super::features::compiled;

pub(super) fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
	attrs
		.iter()
		.find(|(key, _)| key == name)
		.map(|(_, value)| value.as_str())
}

/// What a tag means to the reader.
pub(super) enum Role {
	/// Opens a text block of the named type.
	Text(&'static str, Attrs),
	/// Opens a container of the named type.
	Container(&'static str, Attrs),
	/// An atomic block and its attributes.
	Leaf(&'static str, Attrs),
	/// A line break.
	Break,
	/// An inline atom, such as a mention, whose text is read but not kept.
	Atom(&'static str, Attrs),
	/// Inline formatting: zero or more marks until the tag closes.
	Marks(Vec<(&'static str, Attrs)>),
	/// A block boundary with no type of its own, such as `<div>`.
	Boundary,
	/// Inline markup with no meaning here, such as `<span>`.
	Inline,
}

pub(super) const BOUNDARIES: [&str; 17] = [
	"address",
	"article",
	"aside",
	"body",
	"dd",
	"div",
	"dl",
	"dt",
	"figcaption",
	"figure",
	"footer",
	"header",
	"html",
	"main",
	"nav",
	"section",
	"tbody",
];

impl Role {
	/// The block or embed type the element stands for, if it stands for one.
	fn name(&self) -> Option<&'static str> {
		match self {
			Self::Text(name, _)
			| Self::Container(name, _)
			| Self::Leaf(name, _)
			| Self::Atom(name, _) => Some(name),
			Self::Break | Self::Marks(_) | Self::Boundary | Self::Inline => None,
		}
	}
}

/// A text block's attributes from its `style`: TipTap's text alignment.
fn text_block_attrs(attrs: &[(String, String)]) -> Attrs {
	if !cfg!(feature = "text-align") {
		return Attrs::new();
	}

	let style = attr(attrs, "style").unwrap_or_default();
	// CSS keywords ignore ASCII case: `TEXT-ALIGN: Center` is centred.
	let align = css_value(style, "text-align").map(str::to_ascii_lowercase);

	match align.as_deref() {
		Some(align @ ("left" | "center" | "right" | "justify")) => {
			Attrs::new().with("textAlign", align)
		}
		_ => Attrs::new(),
	}
}

/// What element `name` means inside a container of type `parent`. An
/// element standing for a type whose feature is off reads as plain markup.
pub(super) fn role(name: &str, attrs: &[(String, String)], parent: Option<&'static str>) -> Role {
	let role = element_role(name, attrs, parent);

	if role.name().is_some_and(|name| !compiled(name)) {
		return inline_role(name, attrs);
	}

	role
}

/// What element `name` means with every feature on.
fn element_role(name: &str, attrs: &[(String, String)], parent: Option<&'static str>) -> Role {
	let data_type = attr(attrs, "data-type");

	match name {
		"p" => Role::Text("paragraph", text_block_attrs(attrs)),
		"h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
			let level = i64::from(name.as_bytes()[1] - b'0');
			Role::Text("heading", text_block_attrs(attrs).with("level", level))
		}
		"pre" => Role::Text("codeBlock", Attrs::new()),
		"blockquote" => Role::Container("blockquote", Attrs::new()),
		"ul" => list_role(data_type),
		"ol" => {
			let start = attr(attrs, "start").and_then(|start| start.parse::<i64>().ok());
			Role::Container(
				"orderedList",
				Attrs::new().with("start", start.unwrap_or(1)),
			)
		}
		"li" => {
			list_item_role(
				attrs,
				data_type == Some("taskItem") || parent == Some("taskList"),
			)
		}
		"hr" => Role::Leaf("horizontalRule", Attrs::new()),
		"img" if IMAGE => Role::Leaf("image", media_attrs(attrs, IMAGE_ATTRIBUTES)),
		"audio" if AUDIO => Role::Leaf("audio", audio_attrs(attrs)),
		"iframe" if YOUTUBE || TWITCH => iframe_role(attrs),
		"br" => Role::Break,
		"details" if DETAILS => {
			let open = attr(attrs, "open").is_some();
			Role::Container("details", Attrs::new().with("open", open))
		}
		"summary" if DETAILS => Role::Text("detailsSummary", Attrs::new()),
		"figure" if INK && data_type == Some("ink") => Role::Leaf("ink", ink_attrs(attrs)),
		"table" if TABLE => Role::Container("table", Attrs::new()),
		"tr" if TABLE => Role::Container("tableRow", Attrs::new()),
		"td" if TABLE => Role::Container("tableCell", cell_attrs(attrs)),
		"th" if TABLE => Role::Container("tableHeader", cell_attrs(attrs)),
		"div" => div_role(data_type, attrs),
		"span" => span_role(data_type, attrs),
		_ => inline_role(name, attrs),
	}
}

/// A bulleted list, or a task list when TipTap's `data-type` says so.
fn list_role(data_type: Option<&str>) -> Role {
	match data_type {
		Some("taskList") => Role::Container("taskList", Attrs::new()),
		_ => Role::Container("bulletList", Attrs::new()),
	}
}

/// A list item, or a task item and whether it is checked.
fn list_item_role(attrs: &[(String, String)], task: bool) -> Role {
	if !task {
		return Role::Container("listItem", Attrs::new());
	}

	let checked = attr(attrs, "data-checked") == Some("true");
	Role::Container("taskItem", Attrs::new().with("checked", checked))
}

/// A `<div>`: the first of a file card, details content, a callout, and
/// block math that its attributes make it and whose feature is on, or
/// else a block boundary.
fn div_role(data_type: Option<&str>, attrs: &[(String, String)]) -> Role {
	let candidates = [
		(FILE && data_type == Some("file"))
			.then(|| Role::Leaf("file", media_attrs(attrs, FILE_ATTRIBUTES))),
		(DETAILS && data_type == Some("detailsContent"))
			.then(|| Role::Container("detailsContent", Attrs::new())),
		attr(attrs, "data-callout-type")
			.filter(|_| CALLOUT)
			.map(callout_role),
		(MATHEMATICS && data_type == Some("block-math"))
			.then(|| Role::Leaf("blockMath", atom_attrs(attrs))),
	];

	candidates
		.into_iter()
		.flatten()
		.find(|role| role.name().is_none_or(compiled))
		.unwrap_or(Role::Boundary)
}

/// A callout of type `kind`. An unknown type reads as the default, as the
/// theme draws it.
fn callout_role(kind: &str) -> Role {
	let known = super::markdown_write::ALERTS
		.iter()
		.any(|(alert, _)| *alert == kind);
	let kind = if known { kind } else { "info" };

	Role::Container("callout", Attrs::new().with("type", kind))
}

/// A `<span>`: an inline formula, a mention, or an emoji, as its
/// `data-type` says, or else whatever marks it carries.
fn span_role(data_type: Option<&str>, attrs: &[(String, String)]) -> Role {
	match data_type {
		Some("inline-math") if MATHEMATICS => Role::Atom("inlineMath", atom_attrs(attrs)),
		Some("mention") if MENTION => Role::Atom("mention", atom_attrs(attrs)),
		Some("emoji") if EMOJI => Role::Atom("emoji", atom_attrs(attrs)),
		_ => inline_role("span", attrs),
	}
}

/// The HTML attributes of `<img>`, and the image attributes they hold.
const IMAGE_ATTRIBUTES: &[(&str, &str)] = &[
	("src", "src"),
	("alt", "alt"),
	("title", "title"),
	("width", "width"),
	("height", "height"),
];

/// The HTML attributes of a video `<iframe>` that videos keep.
const VIDEO_ATTRIBUTES: &[(&str, &str)] =
	&[("src", "src"), ("width", "width"), ("height", "height")];

/// The HTML attributes of a file card, and the file attributes they hold.
const FILE_ATTRIBUTES: &[(&str, &str)] = &[
	("data-url", "url"),
	("data-file-name", "fileName"),
	("data-file-type", "fileType"),
	("data-file-size", "fileSize"),
];

/// The boolean attributes of `<audio>`.
const AUDIO_FLAGS: [&str; 4] = ["controls", "autoplay", "loop", "muted"];

/// The attributes `mapping` names, with sizes read as numbers.
fn media_attrs(attrs: &[(String, String)], mapping: &[(&str, &'static str)]) -> Attrs {
	mapping
		.iter()
		.filter_map(|(html, key)| {
			let value = attr(attrs, html)?;
			let number = matches!(*key, "width" | "height" | "fileSize")
				.then(|| value.parse::<f64>().ok())
				.flatten();
			Some((*key, number.map_or_else(|| Value::from(value), Value::from)))
		})
		.collect()
}

/// An `<audio>` element's source and flags; a flag is on when present.
fn audio_attrs(attrs: &[(String, String)]) -> Attrs {
	let flags = AUDIO_FLAGS
		.iter()
		.map(|flag| (*flag, Value::Bool(attr(attrs, flag).is_some())));
	let src = attr(attrs, "src").map(|src| ("src", Value::from(src)));
	src.into_iter().chain(flags).collect()
}

/// An `<iframe>` showing a YouTube or Twitch video, as TipTap writes them;
/// other iframes carry nothing to keep.
fn iframe_role(attrs: &[(String, String)]) -> Role {
	let Some(video) = attr(attrs, "src").and_then(video_kind) else {
		return Role::Inline;
	};

	Role::Leaf(video, media_attrs(attrs, VIDEO_ATTRIBUTES))
}

#[cfg(any(feature = "youtube", feature = "twitch"))]
pub(super) use canvist_model::video_kind;

/// Without the video extensions, no iframe is a video.
#[cfg(not(any(feature = "youtube", feature = "twitch")))]
pub(super) fn video_kind(_src: &str) -> Option<&'static str> {
	None
}

/// The `data-` attributes TipTap writes on inline atoms, and the attributes
/// they hold.
const ATOM_ATTRIBUTES: [(&str, &str); 6] = [
	("data-id", "id"),
	("data-label", "label"),
	("data-mention-suggestion-char", "mentionSuggestionChar"),
	("data-name", "name"),
	("data-emoji", "emoji"),
	("data-latex", "latex"),
];

/// An ink drawing's height and strokes, which `data-strokes` carries as
/// JSON.
fn ink_attrs(attrs: &[(String, String)]) -> Attrs {
	let mut ink = Attrs::new();

	if let Some(height) =
		attr(attrs, "data-height").and_then(|height| height.trim().parse::<i64>().ok())
	{
		ink.set("height", height.max(1));
	}

	let strokes = attr(attrs, "data-strokes")
		.and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
		.map(|json| json_value(&json));

	if let Some(strokes @ Value::Array(_)) = strokes {
		ink.set("strokes", strokes);
	}

	ink
}

/// A JSON value as a document value.
fn json_value(json: &serde_json::Value) -> Value {
	match json {
		serde_json::Value::Null => Value::Null,
		serde_json::Value::Bool(flag) => Value::Bool(*flag),
		serde_json::Value::Number(number) => {
			number.as_i64().map_or_else(
				|| Value::Float(number.as_f64().unwrap_or_default()),
				Value::Int,
			)
		}
		serde_json::Value::String(text) => Value::from(text.as_str()),
		serde_json::Value::Array(items) => {
			Value::from(items.iter().map(json_value).collect::<Vec<_>>())
		}
		serde_json::Value::Object(map) => {
			Value::from(
				map.iter()
					.map(|(key, value)| (key.as_str(), json_value(value)))
					.collect::<Attrs>(),
			)
		}
	}
}

/// The largest spans a cell keeps, the limits browsers apply to
/// `colspan` and `rowspan`, so a pasted span cannot make a table grid of
/// billions of slots.
const MAX_SPANS: [(&str, i64); 2] = [("colspan", 1_000), ("rowspan", 65_534)];

/// A table cell's spans and TipTap's `colwidth`, a comma-separated list of
/// pixel widths.
fn cell_attrs(attrs: &[(String, String)]) -> Attrs {
	let mut cell = Attrs::new();

	for (key, most) in MAX_SPANS {
		if let Some(span) = attr(attrs, key).and_then(|span| span.trim().parse::<i64>().ok()) {
			cell.set(key, span.clamp(1, most));
		}
	}

	let widths: Vec<Value> = attr(attrs, "colwidth")
		.unwrap_or_default()
		.split(',')
		.filter_map(|width| width.trim().parse::<i64>().ok())
		.map(Value::Int)
		.collect();

	if !widths.is_empty() {
		cell.set("colwidth", widths);
	}

	cell
}

fn atom_attrs(attrs: &[(String, String)]) -> Attrs {
	ATOM_ATTRIBUTES
		.iter()
		.filter_map(|(data, key)| Some((*key, attr(attrs, data)?)))
		.collect()
}

/// How an inline element reads: a boundary, plain content, or the marks
/// its tag, `style`, and `data-*` attributes carry.
fn inline_role(name: &str, attrs: &[(String, String)]) -> Role {
	if BOUNDARIES.contains(&name) {
		return Role::Boundary;
	}

	let tagged = tag_mark(name);
	let mut marks = plain_marks(tagged, attrs);
	marks.extend(attribute_marks(name, attrs));

	if marks.is_empty() && tagged.is_none() && name != "span" && name != "a" {
		return Role::Inline;
	}

	Role::Marks(marks)
}

/// The mark an element's tag stands for, when it takes no attributes.
fn tag_mark(name: &str) -> Option<&'static str> {
	match name {
		"strong" | "b" => Some("bold"),
		"em" | "i" => Some("italic"),
		"u" => Some("underline"),
		"s" | "strike" | "del" => Some("strike"),
		"code" => Some("code"),
		"sub" if cfg!(feature = "subscript") => Some("subscript"),
		"sup" if cfg!(feature = "superscript") => Some("superscript"),
		_ => None,
	}
}

/// The tag's mark and the inline `style`'s, less bold when the style sets
/// a normal weight.
fn plain_marks(
	tagged: Option<&'static str>,
	attrs: &[(String, String)],
) -> Vec<(&'static str, Attrs)> {
	let style = attr(attrs, "style").map(Style::parse).unwrap_or_default();

	tagged
		.into_iter()
		.chain(style.marks())
		.filter(|&mark| !(mark == "bold" && style.bold == Some(false)))
		.map(|mark| (mark, Attrs::new()))
		.collect()
}

/// The marks that carry attributes, in order: a link, a hashtag, kinetic
/// type, a comment, a conditional, a highlight, and a text style.
fn attribute_marks(name: &str, attrs: &[(String, String)]) -> Vec<(&'static str, Attrs)> {
	let data_type = attr(attrs, "data-type");
	let css = attr(attrs, "style").unwrap_or_default();

	let link = (name == "a").then(|| {
		let href = attr(attrs, "href").unwrap_or_default();
		("link", Attrs::new().with("href", href))
	});
	let hashtag = (cfg!(feature = "hashtag") && data_type == Some("hashtag")).then(|| {
		let tag = attr(attrs, "data-tag").map_or(Value::Null, Value::from);
		("hashtag", Attrs::new().with("tag", tag))
	});
	let kinetic = cfg!(feature = "kinetic")
		.then(|| attr(attrs, "data-kinetic"))
		.flatten()
		.map(|effect| ("kinetic", Attrs::new().with("effect", effect)));
	let comment = (cfg!(feature = "comments") && data_type == Some("comment")).then(|| {
		let threads: Vec<Value> = attr(attrs, "data-threads")
			.unwrap_or_default()
			.split_whitespace()
			.map(Value::from)
			.collect();
		("comment", Attrs::new().with("threads", threads))
	});
	let conditional = (cfg!(feature = "conditional") && data_type == Some("conditional"))
		.then(|| attr(attrs, "data-when"))
		.flatten()
		.filter(|when| !when.trim().is_empty())
		.map(|when| ("conditional", Attrs::new().with("when", when)));
	let highlight = (cfg!(feature = "highlight") && name == "mark").then(|| {
		let color = attr(attrs, "data-color").or_else(|| css_value(css, "background-color"));
		let attrs = color.map_or_else(Attrs::new, |color| Attrs::new().with("color", color));
		("highlight", attrs)
	});
	let text_style = (cfg!(feature = "text-style") && name != "mark")
		.then(|| text_style_attrs(css))
		.filter(|styles| !styles.is_empty())
		.map(|styles| ("textStyle", styles));

	[
		link,
		hashtag,
		kinetic,
		comment,
		conditional,
		highlight,
		text_style,
	]
	.into_iter()
	.flatten()
	.collect()
}

/// The value of CSS `property` in a `style` attribute.
fn css_value<'a>(style: &'a str, property: &str) -> Option<&'a str> {
	style.split(';').find_map(|declaration| {
		let (name, value) = declaration.split_once(':')?;
		let value = value.trim();
		(name.trim().eq_ignore_ascii_case(property) && !value.is_empty()).then_some(value)
	})
}

/// `textStyle` attributes from a `style` attribute: colour, background,
/// font family, size, and line height. A transparent background is no
/// background.
fn text_style_attrs(style: &str) -> Attrs {
	let mut attrs = Attrs::new();

	for (attr, property) in super::html_write::TEXT_STYLE_PROPERTIES {
		let value = css_value(style, property).filter(|value| *value != "transparent");

		if let Some(value) = value {
			attrs.set(attr, value);
		}
	}

	attrs
}

/// Formatting carried by inline `style` attributes, which word processors
/// such as Google Docs use instead of `<b>` and `<i>`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Style {
	/// `Some(false)` for an explicit normal weight, which cancels `<b>`.
	bold: Option<bool>,
	italic: bool,
	underline: bool,
	strike: bool,
}

impl Style {
	fn parse(style: &str) -> Self {
		let mut parsed = Self::default();

		for declaration in style.split(';') {
			let Some((property, value)) = declaration.split_once(':') else {
				continue;
			};
			let value = value.trim().to_ascii_lowercase();

			match property.trim().to_ascii_lowercase().as_str() {
				"font-weight" => parsed.bold = Some(heavy(&value)),
				"font-style" => parsed.italic = value == "italic" || value == "oblique",
				"text-decoration" | "text-decoration-line" => {
					parsed.underline = value.contains("underline");
					parsed.strike = value.contains("line-through");
				}
				_ => {}
			}
		}

		parsed
	}

	fn marks(self) -> impl Iterator<Item = &'static str> {
		[
			(self.bold == Some(true), "bold"),
			(self.italic, "italic"),
			(self.underline, "underline"),
			(self.strike, "strike"),
		]
		.into_iter()
		.filter_map(|(on, mark)| on.then_some(mark))
	}
}

/// The lightest CSS font weight that reads as bold (semibold).
const BOLD_WEIGHT: u16 = 600;

/// Whether a `font-weight` value is bold.
fn heavy(value: &str) -> bool {
	match value.parse::<u16>() {
		Ok(weight) => weight >= BOLD_WEIGHT,
		Err(_) => value == "bold" || value == "bolder",
	}
}
