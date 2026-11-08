#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_youtube`
//!
//! TipTap's `youtube` block for the canvist editor: a YouTube player the
//! host draws in the slot the canvas reserves. `setYoutubeVideo` inserts
//! one, and refuses any URL not on YouTube's hosts, so nothing else ever
//! reaches an `<iframe>`.
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//! use canvist_extension_youtube::Youtube;
//! use canvist_model::Attrs;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let youtube = builder.add(Youtube);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! let update = editor.run(&youtube.insert("https://youtu.be/dQw4w9WgXcQ", &Attrs::new()), &mut env)?;
//! assert!(update.is_some());
//! # Ok::<(), canvist_core::EditorError>(())
//! ```

use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::Extension;
use canvist_core::NodeMeta;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_extension_media::MediaAttr;
use canvist_extension_media::MediaBlock;
pub use canvist_extension_media::MediaHandle;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::video_kind;

static YOUTUBE: MediaBlock = MediaBlock {
	name: "youtube",
	command: "setYoutubeVideo",
	doc: "Insert a YouTube video at the selection.",
	attrs: &[
		MediaAttr::text("src", "The video URL."),
		MediaAttr::number("start", Some(0), "Where playback starts, in seconds."),
		MediaAttr::number("width", Some(640), "The width in pixels."),
		MediaAttr::number("height", Some(480), "The height in pixels."),
	],
	accepts: Some(|src| video_kind(src) == Some("youtube")),
};

/// TipTap's `youtube` block, with `src`, `start`, `width`, and
/// `height`: `setYoutubeVideo`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Youtube;

impl Extension for Youtube {
	fn name(&self) -> &'static str {
		YOUTUBE.name
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		YOUTUBE.schema(schema)
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		YOUTUBE.nodes()
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(YOUTUBE.commands())
	}
}

impl TypedExtension for Youtube {
	type Handle = MediaHandle;

	fn handle(&self, registration: Registration) -> MediaHandle {
		YOUTUBE.handle(registration)
	}
}
