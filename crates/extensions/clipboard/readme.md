<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://docs.rs/canvist_extension_clipboard"><strong>API</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://crates.io/crates/canvist_extension_clipboard"><img src="https://img.shields.io/crates/v/canvist_extension_clipboard?color=3B2CF5" alt="crates.io version" /></a>
  <a href="https://docs.rs/canvist_extension_clipboard"><img src="https://img.shields.io/docsrs/canvist_extension_clipboard" alt="docs.rs status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_extension_clipboard

HTML, Markdown, and plain-text conversion for the canvist editor's clipboard, reading TipTap, Google Docs, and Word markup, for the [canvist](https://github.com/canvist/canvist) editor.

Most apps get it through [`canvist_extensions`](../../canvist_extensions), which combines every extension behind Cargo features, or [`canvist`](../../canvist), the crate to install. Depend on this crate directly to compile only the extensions you register:

```sh
cargo add canvist_extension_clipboard
```

## Usage

Each reader and writer takes the schema of the editor the content is for:

```rust
use canvist_extension_clipboard::from_html;
use canvist_extension_clipboard::to_markdown;

let schema = editor.schema().clone();
let slice = from_html(&schema, "<p>Some <b>bold</b> text</p>");

assert_eq!(to_markdown(&schema, &slice), "Some **bold** text");
editor.paste(slice, &mut env)?;
```

The StarterKit's markup is always understood. Each Cargo feature, such as `table`, `image`, or `highlight`, adds the markup of the extension of the same name; without it, that markup reads and writes as plain HTML or Markdown. `canvist_extensions` turns each feature on with its extension.

## License

[Unlicense](https://unlicense.org)
