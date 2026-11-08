#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_document`
//!
//! TipTap's `doc` node for the canvist editor, as in TipTap's
//! `@tiptap/extension-document`. [`Document`] is the root, which holds any
//! block in the `block` group. It adds no commands, keys, or rules, and its
//! handle is the plain [`Registration`]. Every editor registers it,
//! alongside at least one text block such as a paragraph so there is
//! somewhere to type.
//!
//! ```
//! use canvist_core::Extension;
//! use canvist_extension_document::Document;
//! use canvist_model::BlockSpec;
//! use canvist_model::Schema;
//!
//! let blocks = Schema::builder().block(BlockSpec::text("paragraph"));
//! let schema = Document.schema(blocks).build()?;
//! let paragraph = schema.block_type("paragraph").expect("a paragraph");
//!
//! assert_eq!(schema.root_children(), [paragraph]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Register it with [`Editor::builder`](canvist_core::Editor::builder),
//! alongside `canvist_extension_paragraph`'s `Paragraph`.

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

#[cfg(test)]
mod tests {
	use canvist_model::BlockSpec;
	use canvist_model::Schema;

	use super::*;

	#[test]
	fn the_root_holds_the_block_group() {
		let builder = Schema::builder()
			.root(&[])
			.block(BlockSpec::text("paragraph"));
		let schema = Document.schema(builder).build().expect("valid schema");
		let paragraph = schema
			.block_type("paragraph")
			.expect("paragraph is registered");

		assert_eq!(schema.root_children(), [paragraph]);
	}
}
