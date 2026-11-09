//! HTML to slices: a forgiving reader for pasted markup.
//!
//! It tokenizes tags and text, keeps the structure and formatting the
//! StarterKit understands, and treats everything else as transparent, so
//! markup from browsers, word processors, and TipTap all paste sensibly.

use canvist_core::Slice;
use canvist_model::Attrs;
use canvist_model::BlockTree;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::MarkSet;
use canvist_model::Schema;
use canvist_model::Value;

use super::NestingLimitExceeded;
use super::html_role::BOUNDARIES;
use super::html_role::Role;
use super::html_role::attr;
use super::html_role::role;
use super::html_token::Token;
use super::html_token::Tokenizer;
use super::tree::Builder;

/// Parse pasted HTML into a slice for `schema`.
/// Input exceeding [`super::MAX_PARSE_DEPTH`] is preserved as literal
/// plain text. Use [`try_from_html`] to reject it instead.
#[must_use]
pub fn from_html(schema: &Schema, html: &str) -> Slice {
	try_from_html(schema, html)
		.unwrap_or_else(|NestingLimitExceeded| super::from_plain_text(schema, html))
}

/// Parse HTML, rejecting input that exceeds [`super::MAX_PARSE_DEPTH`].
///
/// # Errors
/// Returns [`NestingLimitExceeded`] before excessive nesting can exhaust
/// the stack while fitting or disposing of parsed trees.
pub fn try_from_html(schema: &Schema, html: &str) -> Result<Slice, NestingLimitExceeded> {
	let mut build = Builder::new(schema);
	let trees = read_html(&mut build, html);
	build.finish(trees)
}

/// Read HTML using the enclosing Markdown reader's ids and depth budget.
pub(super) fn read_html(build: &mut Builder<'_>, html: &str) -> Vec<BlockTree> {
	if !build.enter() {
		return Vec::new();
	}

	let mut reader = Reader::new(build);

	for token in Tokenizer::new(html) {
		if reader.build.exceeded() {
			break;
		}

		reader.token(token);
	}

	let trees = reader.finish();
	build.leave();
	trees
}

/// Whether any compiled leaf is an element with a closing tag, whose end
/// must not close the container around it. The StarterKit's only leaf,
/// `<hr>`, is void.
const CLOSED_LEAVES: bool = cfg!(any(
	feature = "audio",
	feature = "file",
	feature = "mathematics",
	feature = "twitch",
	feature = "youtube"
));

/// Whether `tag` is a void element, which has no closing tag.
fn is_void(tag: &str) -> bool {
	matches!(
		tag,
		"area"
			| "base"
			| "br"
			| "col"
			| "embed"
			| "hr"
			| "img"
			| "input"
			| "link"
			| "meta"
			| "source"
			| "track"
			| "wbr"
	)
}

/// An atom's attributes, with the text it shows filling in an emoji's
/// missing character or a mention's missing label.
fn shown_attrs(name: &str, mut attrs: Attrs, shown: &str) -> Attrs {
	let key = match name {
		"emoji" => "emoji",
		"mention" => "label",
		_ => return attrs,
	};

	if attrs.get(key).is_some() {
		return attrs;
	}

	if name == "emoji" && !shown.is_empty() {
		attrs.set(key, shown);
	}

	if name == "mention" {
		let label: String = shown.chars().skip(1).collect();
		// A mention without a label shows its id, which is not a label.
		let id = attrs.get("id").and_then(Value::as_str);

		if id != Some(label.as_str()) {
			attrs.set(key, label);
		}
	}

	attrs
}

/// An open container.
struct Frame {
	tag: String,
	name: &'static str,
	attrs: Attrs,
	children: Vec<BlockTree>,
}

/// The text block being filled.
struct OpenText {
	tag: Option<String>,
	name: &'static str,
	attrs: Attrs,
	inline: InlineText,
	/// Whitespace seen but not yet written, with the marks it had, so
	/// blocks never end in spaces and a space before `<b>` stays unbolded.
	space: Option<MarkSet>,
}

/// An inline atom being read, such as a mention: the text inside its tag
/// is what it shows, not document text.
struct OpenAtom {
	tag: String,
	name: &'static str,
	attrs: Attrs,
	text: String,
}

/// Builds block trees from tokens.
struct Reader<'b, 's> {
	build: &'b mut Builder<'s>,
	frames: Vec<Frame>,
	text: Option<OpenText>,
	/// Open inline tags and the marks each applies.
	marks: Vec<(String, Vec<Mark>)>,
	atom: Option<OpenAtom>,
	/// The tags of open leaf elements that have a closing tag, such as
	/// `<div data-type="block-math">`, so their `</div>` never closes the
	/// container around them.
	leaves: Vec<String>,
}

impl<'b, 's> Reader<'b, 's> {
	fn new(build: &'b mut Builder<'s>) -> Self {
		let root = Frame {
			tag: String::new(),
			name: "",
			attrs: Attrs::new(),
			children: Vec::new(),
		};

		Self {
			build,
			frames: vec![root],
			text: None,
			marks: Vec::new(),
			atom: None,
			leaves: Vec::new(),
		}
	}

	fn in_code(&self) -> bool {
		self.text
			.as_ref()
			.is_some_and(|text| text.name == "codeBlock")
	}

	fn token(&mut self, token: Token) {
		match token {
			Token::Open { name, attrs } => self.open(name, &attrs),
			Token::Close(name) => self.close(&name),
			Token::Text(text) => self.push_text(&text),
		}
	}

	/// Open what tag `tag` starts, as its role says. Blocks end the open
	/// text block first; inline roles add to it.
	fn open(&mut self, tag: String, attrs: &[(String, String)]) {
		let parent = self.frames.last().map(|frame| frame.name);

		match role(&tag, attrs, parent) {
			Role::Text(name, block_attrs) => {
				self.end_text();
				self.text = Some(OpenText {
					tag: Some(tag),
					name,
					attrs: block_attrs,
					inline: InlineText::new(),
					space: None,
				});
			}
			Role::Container(name, block_attrs) => self.open_frame(tag, name, block_attrs),
			Role::Leaf(name, leaf_attrs) => self.add_leaf(tag, name, &leaf_attrs),
			Role::Break => self.line_break(),
			Role::Atom(name, attrs) => {
				self.atom = Some(OpenAtom {
					tag,
					name,
					attrs,
					text: String::new(),
				});
			}
			Role::Marks(marks) => self.open_marks(tag, marks, attrs),
			Role::Boundary => self.end_text(),
			Role::Inline => {}
		}
	}

	/// Open a container, unless that would nest too deeply.
	fn open_frame(&mut self, tag: String, name: &'static str, attrs: Attrs) {
		if !self.build.enter() {
			return;
		}

		self.end_text();
		self.frames.push(Frame {
			tag,
			name,
			attrs,
			children: Vec::new(),
		});
	}

	/// Add a leaf block, and remember its tag until it closes when it is an
	/// element with a closing tag.
	fn add_leaf(&mut self, tag: String, name: &'static str, attrs: &Attrs) {
		self.end_text();
		let leaf = self.build.leaf(name, attrs);
		self.children().extend(leaf);

		if CLOSED_LEAVES && !is_void(&tag) {
			self.leaves.push(tag);
		}
	}

	/// Apply an inline element's marks until it closes. Inside code, marks
	/// are dropped, and the element only carries the code's language, as
	/// `<code class="language-rust">` inside `<pre>` does.
	fn open_marks(
		&mut self,
		tag: String,
		marks: Vec<(&'static str, Attrs)>,
		attrs: &[(String, String)],
	) {
		if self.in_code() {
			let language = attr(attrs, "class").and_then(|class| class.strip_prefix("language-"));
			let text = self.text.as_mut().zip(language);

			if let Some((text, language)) = text {
				text.attrs.set("language", Value::from(language));
			}

			return;
		}

		// A text style means only its attributes, so it needs one the schema
		// declares.
		let marks = marks
			.into_iter()
			.filter_map(|(name, mark_attrs)| {
				match name {
					"textStyle" => self.build.style_mark(name, &mark_attrs),
					_ => self.build.mark(name, &mark_attrs),
				}
			})
			.collect();

		if self.build.enter() {
			self.marks.push((tag, marks));
		}
	}

	/// Close the innermost open thing `tag` closes, checked in order: the
	/// open atom, a leaf, the text block, a mark, then a frame and every
	/// frame inside it. A boundary tag only ends the text.
	fn close(&mut self, tag: &str) {
		if self.atom.as_ref().is_some_and(|atom| atom.tag == tag) {
			self.finish_atom();

			return;
		}

		let closed = self.close_leaf(tag)
			|| self.close_text(tag)
			|| self.close_mark(tag)
			|| self.close_frame(tag);

		if !closed && BOUNDARIES.contains(&tag) {
			self.end_text();
		}
	}

	/// Close the innermost open leaf element `tag`, if there is one.
	fn close_leaf(&mut self, tag: &str) -> bool {
		let index = CLOSED_LEAVES
			.then(|| self.leaves.iter().rposition(|open| open == tag))
			.flatten();

		index
			.inspect(|&index| self.leaves.truncate(index))
			.is_some()
	}

	/// End the open text block, if `tag` opened it.
	fn close_text(&mut self, tag: &str) -> bool {
		let closes = self
			.text
			.as_ref()
			.is_some_and(|text| text.tag.as_deref() == Some(tag));

		if closes {
			self.end_text();
		}

		closes
	}

	/// Remove the marks of the innermost open inline element `tag`.
	fn close_mark(&mut self, tag: &str) -> bool {
		let Some(index) = self.marks.iter().rposition(|(open, _)| open == tag) else {
			return false;
		};

		self.marks.remove(index);
		self.build.leave();
		true
	}

	/// Close the innermost container `tag` and every container inside it.
	fn close_frame(&mut self, tag: &str) -> bool {
		let index = self
			.frames
			.iter()
			.rposition(|frame| frame.tag == tag && !tag.is_empty());
		let Some(index) = index else {
			return false;
		};

		self.end_text();

		while self.frames.len() > index.max(1) {
			self.pop_frame();
		}

		true
	}

	fn children(&mut self) -> &mut Vec<BlockTree> {
		let last = self.frames.len() - 1;
		&mut self.frames[last].children
	}

	fn pop_frame(&mut self) {
		let Some(mut frame) = self.frames.pop() else {
			return;
		};
		self.build.leave();

		// A cell always holds a block, so an empty one keeps its place.
		if cfg!(feature = "table")
			&& frame.children.is_empty()
			&& matches!(frame.name, "tableCell" | "tableHeader")
		{
			let paragraph = self
				.build
				.text("paragraph", Attrs::new(), InlineText::new());
			frame.children.push(paragraph);
		}

		let trees = self
			.build
			.container(frame.name, &frame.attrs, frame.children);
		self.children().extend(trees);
	}

	fn marks(&self) -> MarkSet {
		MarkSet::from_marks(
			self.marks
				.iter()
				.flat_map(|(_, marks)| marks.iter().cloned()),
		)
	}

	/// The open text block, opening an implicit paragraph if needed.
	fn ensure_text(&mut self) -> &mut OpenText {
		self.text.get_or_insert_with(|| {
			OpenText {
				tag: None,
				name: "paragraph",
				attrs: Attrs::new(),
				inline: InlineText::new(),
				space: None,
			}
		})
	}

	/// Add text the way a browser renders it: runs of whitespace collapse
	/// to one space, and a space waits until more text follows, so blocks
	/// never start or end with one. Code keeps its text as is.
	fn push_text(&mut self, raw: &str) {
		if let Some(atom) = self.atom.as_mut() {
			atom.text.push_str(raw);

			return;
		}

		if self.in_code() {
			let marks = self.marks();
			self.ensure_text()
				.inline
				.append(InlineText::plain(raw, marks));

			return;
		}

		let words: Vec<&str> = raw.split_ascii_whitespace().collect();
		let leading = raw.starts_with(|character: char| character.is_ascii_whitespace());
		let trailing = raw.ends_with(|character: char| character.is_ascii_whitespace());
		let marks = self.marks();

		if words.is_empty() {
			if let Some(text) = self.text.as_mut().filter(|_| leading) {
				text.space.get_or_insert(marks);
			}

			return;
		}

		let text = self.ensure_text();
		let pending = text.space.take().or_else(|| leading.then(|| marks.clone()));

		if let Some(space_marks) = pending.filter(|_| !text.inline.is_empty()) {
			text.inline.append(InlineText::plain(" ", space_marks));
		}

		text.inline
			.append(InlineText::plain(&words.join(" "), marks.clone()));
		text.space = trailing.then_some(marks);
	}

	/// Add the atom just closed: as an embed, with what its text shows
	/// filling in a missing label or emoji, or as that text when the schema
	/// has no such embed.
	fn finish_atom(&mut self) {
		let Some(atom) = self.atom.take() else {
			return;
		};
		let attrs = shown_attrs(atom.name, atom.attrs, atom.text.trim());
		let marks = self.marks();
		let Some(embed) = self.build.embed(atom.name, &attrs, marks) else {
			self.push_text(&atom.text);

			return;
		};
		let text = self.ensure_text();

		if let Some(space) = text.space.take().filter(|_| !text.inline.is_empty()) {
			text.inline.append(InlineText::plain(" ", space));
		}

		text.inline.append(embed);
	}

	fn line_break(&mut self) {
		if self.in_code() {
			self.push_text("\n");

			return;
		}

		let marks = self.marks();

		match self.build.hard_break(marks) {
			Some(embed) => {
				let text = self.ensure_text();
				text.inline.append(embed);
				text.space = None;
			}
			None => self.end_text(),
		}
	}

	fn end_text(&mut self) {
		let Some(text) = self.text.take() else {
			return;
		};

		if text.tag.is_none() && text.inline.is_empty() {
			return;
		}

		let block = self.build.text(text.name, text.attrs, text.inline);
		self.children().push(block);
	}

	fn finish(mut self) -> Vec<BlockTree> {
		self.end_text();

		while self.frames.len() > 1 {
			self.pop_frame();
		}

		for _ in &self.marks {
			self.build.leave();
		}

		self.frames
			.pop()
			.map(|root| root.children)
			.unwrap_or_default()
	}
}
