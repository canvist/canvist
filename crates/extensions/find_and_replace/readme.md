<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://docs.rs/canvist_extension_find_and_replace"><strong>API</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://crates.io/crates/canvist_extension_find_and_replace"><img src="https://img.shields.io/crates/v/canvist_extension_find_and_replace?color=3B2CF5" alt="crates.io version" /></a>
  <a href="https://docs.rs/canvist_extension_find_and_replace"><img src="https://img.shields.io/docsrs/canvist_extension_find_and_replace" alt="docs.rs status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_extension_find_and_replace

Search a canvist document, step through matches, and replace them, for the [canvist](https://github.com/canvist/canvist) editor.

Most apps get it through [`canvist_extensions`](../../canvist_extensions), which combines every extension behind Cargo features, or [`canvist`](../../canvist), the crate to install. Depend on this crate directly to compile only the extensions you register:

```sh
cargo add canvist_extension_find_and_replace
```

## Usage

```rust
use std::time::Duration;

use canvist_core::Editor;
use canvist_core::Env;
use canvist_extension_document::Document;
use canvist_extension_find_and_replace::FindAndReplace;
use canvist_extension_paragraph::Paragraph;

let mut builder = Editor::builder().extension(Document).extension(Paragraph);
let find = builder.add(FindAndReplace);
let mut editor = builder.build()?;
let mut env = Env::headless(Duration::ZERO);

editor.insert_text("Cats chase cats.", &mut env)?;
editor.run(&find.set_term("cat"), &mut env)?;
assert_eq!(find.matches(editor.state()).len(), 2);

editor.run(&find.replace_all("dog"), &mut env)?;
assert_eq!(editor.state().doc().plain_text(), "dogs chase dogs.");
```

## License

[Unlicense](https://unlicense.org)
