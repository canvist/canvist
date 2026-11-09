<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://docs.rs/canvist_extension_suggestion"><strong>API</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://crates.io/crates/canvist_extension_suggestion"><img src="https://img.shields.io/crates/v/canvist_extension_suggestion?color=3B2CF5" alt="crates.io version" /></a>
  <a href="https://docs.rs/canvist_extension_suggestion"><img src="https://img.shields.io/docsrs/canvist_extension_suggestion" alt="docs.rs status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_extension_suggestion

Trigger-character suggestions (`@`, `#`, `:`) that mention, hashtag, and emoji extensions build on, like TipTap's `@tiptap/suggestion`, for the [canvist](https://github.com/canvist/canvist) editor.

Most apps get it through [`canvist_extensions`](../../canvist_extensions), which combines every extension behind Cargo features, or [`canvist`](../../canvist), the crate to install. Depend on this crate directly to compile only the extensions you register:

```sh
cargo add canvist_extension_suggestion
```

## Usage

```rust
use std::sync::Arc;

use canvist_core::EditorState;
use canvist_core::commands;
use canvist_extension_suggestion::Trigger;
use canvist_model::BlockSpec;
use canvist_model::Document;
use canvist_model::InlineText;
use canvist_model::MarkSet;
use canvist_model::Schema;

const MENTION: Trigger = Trigger { name: "mention", char: '@', allow_spaces: false };

let schema = Arc::new(Schema::builder().block(BlockSpec::text("paragraph")).build()?);
let mut state = EditorState::new(Document::new(schema, 1))?;
commands::insert_text(&mut state, "Hi @ad")?;

let found = MENTION.find(&state).expect("a suggestion");
assert_eq!(found.query, "ad");
assert_eq!(found.range, 3..6);

let picked = InlineText::plain("@Ada", MarkSet::empty());
canvist_extension_suggestion::replace(&mut state, Some(found), picked)?;
assert_eq!(state.doc().plain_text(), "Hi @Ada ");
assert!(MENTION.find(&state).is_none());
```

## License

[Unlicense](https://unlicense.org)
