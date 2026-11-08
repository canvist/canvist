<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://docs.rs/canvist_extension_ink"><strong>API</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://crates.io/crates/canvist_extension_ink"><img src="https://img.shields.io/crates/v/canvist_extension_ink?color=3B2CF5" alt="crates.io version" /></a>
  <a href="https://docs.rs/canvist_extension_ink"><img src="https://img.shields.io/docsrs/canvist_extension_ink" alt="docs.rs status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_extension_ink

An `ink` block holding freehand strokes that the canvist canvas draws, for the [canvist](https://github.com/canvist/canvist) editor.

Most apps get it through [`canvist_extensions`](../../canvist_extensions), which combines every extension behind Cargo features, or [`canvist`](../../canvist), the crate to install. Depend on this crate directly to compile only the extensions you register:

```sh
cargo add canvist_extension_ink
```

## Usage

```rust
use std::time::Duration;

use canvist_core::Editor;
use canvist_core::Env;
use canvist_extension_document::Document;
use canvist_extension_ink::DEFAULT_HEIGHT;
use canvist_extension_ink::Ink;
use canvist_extension_ink::InkStroke;
use canvist_extension_paragraph::Paragraph;

let mut builder = Editor::builder().extension(Document).extension(Paragraph);
let ink = builder.add(Ink);
let mut editor = builder.build()?;
let mut env = Env::headless(Duration::ZERO);

editor.run(&ink.insert(DEFAULT_HEIGHT), &mut env)?;
let drawing = ink.drawings(editor.state())[0];
let stroke = InkStroke {
	points: &[10.0, 20.0, 200.0, 120.0],
	color: Some("#3b2cf5"),
	width: None,
};
editor.run(&ink.add_stroke(drawing, &stroke)?, &mut env)?;

assert!(editor.can(&ink.undo_stroke(drawing), &mut env)?);
```

## License

[Unlicense](https://unlicense.org)
