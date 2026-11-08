#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_file`
//!
//! Remirror's `file` block for the canvist editor: an attachment card the
//! host draws in the slot the canvas reserves, with the file's name, type,
//! and size. Insert one with `insertFile`.
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//! use canvist_extension_file::File;
//! use canvist_model::Attrs;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let file = builder.add(File);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! let update = editor.run(&file.insert("https://example.com/report.pdf", &Attrs::new()), &mut env)?;
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

static FILE: MediaBlock = MediaBlock {
	name: "file",
	command: "insertFile",
	doc: "Insert a file attachment at the selection.",
	attrs: &[
		MediaAttr::text("url", "Where the file can be downloaded."),
		MediaAttr::text("fileName", "The file's name, such as `report.pdf`."),
		MediaAttr::text(
			"fileType",
			"The file's media type, such as `application/pdf`.",
		),
		MediaAttr::number("fileSize", Some(0), "The file's size in bytes."),
	],
	accepts: None,
};

/// Remirror's `file` attachment card, with `url`, `fileName`,
/// `fileType`, and `fileSize`: `insertFile`.
#[derive(Clone, Copy, Debug, Default)]
pub struct File;

impl Extension for File {
	fn name(&self) -> &'static str {
		FILE.name
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		FILE.schema(schema)
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		FILE.nodes()
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(FILE.commands())
	}
}

impl TypedExtension for File {
	type Handle = MediaHandle;

	fn handle(&self, registration: Registration) -> MediaHandle {
		FILE.handle(registration)
	}
}
