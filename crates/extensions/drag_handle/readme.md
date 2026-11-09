<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://docs.rs/canvist_extension_drag_handle"><strong>API</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://crates.io/crates/canvist_extension_drag_handle"><img src="https://img.shields.io/crates/v/canvist_extension_drag_handle?color=3B2CF5" alt="crates.io version" /></a>
  <a href="https://docs.rs/canvist_extension_drag_handle"><img src="https://img.shields.io/docsrs/canvist_extension_drag_handle" alt="docs.rs status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_extension_drag_handle

Move blocks up and down with `Alt-↑` and `Alt-↓`, or to any position, like TipTap's DragHandle, for the [canvist](https://github.com/canvist/canvist) editor.

Most apps get it through [`canvist_extensions`](../../canvist_extensions), which combines every extension behind Cargo features, or [`canvist`](../../canvist), the crate to install. Depend on this crate directly to compile only the extensions you register:

```sh
cargo add canvist_extension_drag_handle
```

## Usage

```rust
use std::time::Duration;

use canvist_core::Editor;
use canvist_core::Env;
use canvist_core::command;
use canvist_core::commands;
use canvist_extension_document::Document;
use canvist_extension_drag_handle::DragHandle;
use canvist_extension_paragraph::Paragraph;

let mut builder = Editor::builder().extension(Document).extension(Paragraph);
let drag = builder.add(DragHandle);
let mut editor = builder.build()?;
let mut env = Env::headless(Duration::ZERO);

editor.insert_text("first", &mut env)?;
editor.run(&command(commands::split_block), &mut env)?;
editor.insert_text("second", &mut env)?;
editor.run(&drag.up(), &mut env)?;

let doc = editor.state().doc();
let first = doc.first_text_block().and_then(|id| doc.inline(id));
assert_eq!(first.map(|text| text.as_str()), Some("second"));
```

## License

[Unlicense](https://unlicense.org)
