<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://docs.rs/canvist_extension_media"><strong>API</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://crates.io/crates/canvist_extension_media"><img src="https://img.shields.io/crates/v/canvist_extension_media?color=3B2CF5" alt="crates.io version" /></a>
  <a href="https://docs.rs/canvist_extension_media"><img src="https://img.shields.io/docsrs/canvist_extension_media" alt="docs.rs status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_extension_media

The building blocks of media extensions: atomic blocks whose attributes say what to show, inserted by one command, for the [canvist](https://github.com/canvist/canvist) editor.

Most apps get it through [`canvist_extensions`](../../canvist_extensions), which combines every extension behind Cargo features, or [`canvist`](../../canvist), the crate to install. Depend on this crate directly to compile only the extensions you register:

```sh
cargo add canvist_extension_media
```

## Usage

```rust
use std::time::Duration;

use canvist_core::CommandSpec;
use canvist_core::Editor;
use canvist_core::EditorError;
use canvist_core::Env;
use canvist_core::Extension;
use canvist_core::NodeMeta;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_extension_document::Document;
use canvist_extension_media::MediaAttr;
use canvist_extension_media::MediaBlock;
use canvist_extension_media::MediaHandle;
use canvist_extension_paragraph::Paragraph;
use canvist_model::Attrs;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;

static VIMEO: MediaBlock = MediaBlock {
	name: "vimeo",
	command: "setVimeoVideo",
	doc: "Insert a Vimeo video at the selection.",
	attrs: &[
		MediaAttr::text("src", "The video URL."),
		MediaAttr::number("width", Some(640), "The width in pixels."),
	],
	accepts: Some(|src| src.starts_with("https://vimeo.com/")),
};

struct Vimeo;

impl Extension for Vimeo {
	fn name(&self) -> &'static str {
		VIMEO.name
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		VIMEO.schema(schema)
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		VIMEO.nodes()
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(VIMEO.commands())
	}
}

impl TypedExtension for Vimeo {
	type Handle = MediaHandle;

	fn handle(&self, registration: Registration) -> MediaHandle {
		VIMEO.handle(registration)
	}
}

let mut builder = Editor::builder().extension(Document).extension(Paragraph);
let vimeo = builder.add(Vimeo);
let mut editor = builder.build()?;
let mut env = Env::headless(Duration::ZERO);

let inserted = editor.run(&vimeo.insert("https://vimeo.com/1", &Attrs::new()), &mut env)?;
assert!(inserted.is_some());

let refused = editor.run(&vimeo.insert("javascript:alert(1)", &Attrs::new()), &mut env)?;
assert!(refused.is_none());
```

## License

[Unlicense](https://unlicense.org)
