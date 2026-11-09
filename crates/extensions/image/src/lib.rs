#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_image`
//!
//! TipTap's `image` block for the canvist editor, as in
//! `@tiptap/extension-image`: a picture the host draws in the slot the
//! canvas reserves. [`Image`] adds the `image` block, with `src`, `alt`,
//! `title`, `width`, and `height`, and the `setImage` command. Typing
//! `![alt](src "title")` inserts one too, as Markdown writes it. Its handle
//! is a [`MediaHandle`].
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_image::Image;
//! use canvist_extension_paragraph::Paragraph;
//! use canvist_model::Attrs;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let image = builder.add(Image);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! let update = editor.run(&image.insert("https://example.com/cat.png", &Attrs::new()), &mut env)?;
//! assert!(update.is_some());
//!
//! for character in "![A dog](dog.png)".chars() {
//! 	editor.insert_text(&character.to_string(), &mut env)?;
//! }
//! let images = editor.state().doc().iter().filter(|block| block.attrs().get("src").is_some());
//! assert_eq!(images.count(), 2);
//! # Ok::<(), canvist_core::EditorError>(())
//! ```

use canvist_core::CommandResult;
use canvist_core::CommandSpec;
use canvist_core::EditKind;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::InputRule;
use canvist_core::NodeMeta;
use canvist_core::Registration;
use canvist_core::RuleMatch;
use canvist_core::TypedExtension;
use canvist_core::block_type;
use canvist_core::commands;
use canvist_extension_media::MediaAttr;
use canvist_extension_media::MediaBlock;
pub use canvist_extension_media::MediaHandle;
use canvist_model::Attrs;
use canvist_model::BlockTypeId;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;

static IMAGE: MediaBlock = MediaBlock {
	name: "image",
	command: "setImage",
	doc: "Insert an image at the selection.",
	attrs: &[
		MediaAttr::text("src", "The image URL."),
		MediaAttr::text("alt", "Text describing the image, for screen readers."),
		MediaAttr::text("title", "A title shown on hover."),
		MediaAttr::number("width", None, "The width in pixels."),
		MediaAttr::number("height", None, "The height in pixels."),
	],
	accepts: None,
};

/// `![alt](src "title")`, as typed: group 2 is the alt text, 3 the source,
/// and 4 the title. The title's quotes may already be curly, when
/// typography is on.
const IMAGE_SYNTAX: &str = r#"(?:^|\s)(!\[([^\]]*)\]\((\S+)(?:\s+["'“‘]([^"'“”‘’]+)["'”’])?\))$"#;

/// Replace typed `![alt](src "title")` with an image block.
fn typed_image(state: &mut EditorState, found: &RuleMatch, kind: BlockTypeId) -> CommandResult {
	let (Some(Some(range)), Some(src)) = (found.groups.get(1).cloned(), found.group(3)) else {
		return Ok(None);
	};
	let given = [("alt", found.group(2)), ("title", found.group(4))];
	let attrs: Attrs = given
		.into_iter()
		.filter_map(|(key, text)| Some((key, text.filter(|text| !text.is_empty())?)))
		.chain([("src", src)])
		.collect();

	state.atomic(|state| {
		let removed = state.change(EditKind::InputRule, |tr| {
			tr.delete_text(found.block, range.start..found.caret)?;
			Ok(true)
		})?;
		let inserted = commands::insert_leaf(state, kind, attrs)?;

		Ok(match (removed, inserted) {
			(Some(removed), Some(inserted)) => Some(removed.then(inserted)),
			_ => None,
		})
	})
}

/// TipTap's `image` block, with `src`, `alt`, `title`, `width`, and
/// `height`: `setImage`, or `![alt](src "title")` typed at the start of a
/// line or after white space.
#[derive(Clone, Copy, Debug, Default)]
pub struct Image;

impl Extension for Image {
	fn name(&self) -> &'static str {
		IMAGE.name
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		IMAGE.schema(schema)
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		IMAGE.nodes()
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(IMAGE.commands())
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let kind = block_type(schema, IMAGE.name, IMAGE.name)?;
		let rule = InputRule::new(IMAGE_SYNTAX, move |state, found| {
			typed_image(state, found, kind)
		})?;
		Ok(vec![rule.on(")")])
	}
}

impl TypedExtension for Image {
	type Handle = MediaHandle;

	fn handle(&self, registration: Registration) -> MediaHandle {
		IMAGE.handle(registration)
	}
}

#[cfg(test)]
mod tests {
	use std::time::Duration;

	use canvist_core::Editor;
	use canvist_core::Env;
	use canvist_extension_document::Document;
	use canvist_extension_paragraph::Paragraph;
	use canvist_model::Value;

	use super::*;

	/// The attributes of every image after typing `text`, and the text left.
	fn typed(text: &str) -> Result<(Vec<Attrs>, String), EditorError> {
		let mut editor = Editor::builder()
			.extension(Document)
			.extension(Paragraph)
			.extension(Image)
			.build()?;
		let mut env = Env::headless(Duration::ZERO);

		for character in text.chars() {
			editor.insert_text(&character.to_string(), &mut env)?;
		}

		let doc = editor.state().doc();
		let kind = doc.schema().block_type(IMAGE.name);
		let images = doc
			.iter()
			.filter(|block| Some(block.kind()) == kind)
			.map(|block| block.attrs().clone())
			.collect();
		Ok((images, doc.plain_text()))
	}

	#[test]
	fn typed_syntax_keeps_alt_and_title_and_drops_empty_alt() -> Result<(), EditorError> {
		let (images, _) = typed("![A cat](cat.png \"Meow\")")?;
		let cat = Attrs::new()
			.with("alt", "A cat")
			.with("src", "cat.png")
			.with("title", "Meow");
		assert_eq!(images, [cat]);

		let (images, _) = typed("![](x.png)")?;
		assert_eq!(images, [Attrs::new().with("src", "x.png")]);
		Ok(())
	}

	#[test]
	fn typed_titles_may_have_curly_quotes() -> Result<(), EditorError> {
		let (images, _) = typed("![a](b.png “Curly”)")?;
		let title = images.first().and_then(|attrs| attrs.get("title").cloned());

		assert_eq!(title, Some(Value::from("Curly")));
		Ok(())
	}

	#[test]
	fn typed_syntax_needs_white_space_before_it_and_none_in_the_source() -> Result<(), EditorError>
	{
		assert_eq!(
			typed("x![a](b.png)")?,
			(Vec::new(), "x![a](b.png)".to_owned())
		);
		assert_eq!(typed("![a](b c.png)")?.0, []);
		Ok(())
	}
}
