#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_twitch`
//!
//! TipTap's `twitch` block for the canvist editor, as in
//! `@tiptap/extension-twitch`: a Twitch player the host draws in the slot
//! the canvas reserves. [`Twitch`] adds the `twitch` block, with `src`,
//! `width`, `height`, `autoplay`, and `muted`, and the `setTwitchVideo`
//! command, which refuses any URL not on Twitch, so nothing else ever
//! reaches an `<iframe>`. Its handle is a [`MediaHandle`].
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//! use canvist_extension_twitch::Twitch;
//! use canvist_model::Attrs;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let twitch = builder.add(Twitch);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! let update = editor.run(&twitch.insert("https://www.twitch.tv/videos/1", &Attrs::new()), &mut env)?;
//! assert!(update.is_some());
//!
//! let refused = editor.run(&twitch.insert("https://youtu.be/x", &Attrs::new()), &mut env)?;
//! assert!(refused.is_none());
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

/// The player's width and height in pixels, as TipTap's defaults.
const SIZE: (u16, u16) = (640, 480);

static TWITCH: MediaBlock = MediaBlock {
	name: "twitch",
	command: "setTwitchVideo",
	doc: "Insert a Twitch video, clip, or channel at the selection.",
	attrs: &[
		MediaAttr::text("src", "The video, clip, or channel URL."),
		MediaAttr::number("width", Some(SIZE.0), "The width in pixels."),
		MediaAttr::number("height", Some(SIZE.1), "The height in pixels."),
		MediaAttr::flag("autoplay", false, "Start playing straight away."),
		MediaAttr::flag("muted", false, "Start muted."),
	],
	accepts: Some(|src| video_kind(src) == Some("twitch")),
};

/// TipTap's `twitch` block, with `src`, `width`, `height`, `autoplay`,
/// and `muted`: `setTwitchVideo`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Twitch;

impl Extension for Twitch {
	fn name(&self) -> &'static str {
		TWITCH.name
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		TWITCH.schema(schema)
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		TWITCH.nodes()
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(TWITCH.commands())
	}
}

impl TypedExtension for Twitch {
	type Handle = MediaHandle;

	fn handle(&self, registration: Registration) -> MediaHandle {
		TWITCH.handle(registration)
	}
}
