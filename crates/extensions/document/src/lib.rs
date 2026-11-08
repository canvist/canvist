#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_document`
//!
//! TipTap's `doc` node for the canvist editor: the root, which holds any
//! block in the `block` group. Every editor registers it.
//!
//! ```
//! use canvist_core::Editor;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//!
//! let editor = Editor::builder().extension(Document).extension(Paragraph).build()?;
//!
//! assert_eq!(editor.state().doc().plain_text(), "");
//! # Ok::<(), canvist_core::EditorError>(())
//! ```

use canvist_core::Extension;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_model::SchemaBuilder;

/// The document root, which holds any block in the `block` group.
#[derive(Clone, Copy, Debug, Default)]
pub struct Document;

impl Extension for Document {
	fn name(&self) -> &'static str {
		"doc"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.root(&["block"])
	}
}

impl TypedExtension for Document {
	type Handle = Registration;

	fn handle(&self, registration: Registration) -> Registration {
		registration
	}
}
