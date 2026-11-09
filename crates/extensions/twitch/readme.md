<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://docs.rs/canvist_extension_twitch"><strong>API</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://crates.io/crates/canvist_extension_twitch"><img src="https://img.shields.io/crates/v/canvist_extension_twitch?color=3B2CF5" alt="crates.io version" /></a>
  <a href="https://docs.rs/canvist_extension_twitch"><img src="https://img.shields.io/docsrs/canvist_extension_twitch" alt="docs.rs status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_extension_twitch

TipTap's `twitch` block, embedding only Twitch URLs, for the [canvist](https://github.com/canvist/canvist) editor.

Most apps get it through [`canvist_extensions`](../../canvist_extensions), which combines every extension behind Cargo features, or [`canvist`](../../canvist), the crate to install. Depend on this crate directly to compile only the extensions you register:

```sh
cargo add canvist_extension_twitch
```

## Usage

```rust
use std::time::Duration;

use canvist_core::Editor;
use canvist_core::Env;
use canvist_extension_document::Document;
use canvist_extension_paragraph::Paragraph;
use canvist_extension_twitch::Twitch;
use canvist_model::Attrs;

let mut builder = Editor::builder().extension(Document).extension(Paragraph);
let twitch = builder.add(Twitch);
let mut editor = builder.build()?;
let mut env = Env::headless(Duration::ZERO);

let update = editor.run(&twitch.insert("https://www.twitch.tv/videos/1", &Attrs::new()), &mut env)?;
assert!(update.is_some());

let refused = editor.run(&twitch.insert("https://youtu.be/x", &Attrs::new()), &mut env)?;
assert!(refused.is_none());
```

## License

[Unlicense](https://unlicense.org)
