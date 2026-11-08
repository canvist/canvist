#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_audio`
//!
//! TipTap's `audio` block for the canvist editor: an audio player the host
//! draws in the slot the canvas reserves. [`Audio`] adds the `audio` block,
//! with `src`, `controls`, `autoplay`, `loop`, and `muted`, and the
//! `setAudio` command. Its handle is a [`MediaHandle`].
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//! use canvist_extension_audio::Audio;
//! use canvist_model::Attrs;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let audio = builder.add(Audio);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! let update = editor.run(&audio.insert("https://example.com/song.mp3", &Attrs::new()), &mut env)?;
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

static AUDIO: MediaBlock = MediaBlock {
	name: "audio",
	command: "setAudio",
	doc: "Insert an audio player at the selection.",
	attrs: &[
		MediaAttr::text("src", "The audio URL."),
		MediaAttr::flag("controls", true, "Show playback controls."),
		MediaAttr::flag("autoplay", false, "Start playing straight away."),
		MediaAttr::flag("loop", false, "Play again from the start when it ends."),
		MediaAttr::flag("muted", false, "Start muted."),
	],
	accepts: None,
};

/// TipTap's `audio` block, with `src`, `controls`, `autoplay`, `loop`,
/// and `muted`: `setAudio`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Audio;

impl Extension for Audio {
	fn name(&self) -> &'static str {
		AUDIO.name
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		AUDIO.schema(schema)
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		AUDIO.nodes()
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(AUDIO.commands())
	}
}

impl TypedExtension for Audio {
	type Handle = MediaHandle;

	fn handle(&self, registration: Registration) -> MediaHandle {
		AUDIO.handle(registration)
	}
}
