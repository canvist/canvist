---
"canvist_core": feat
"canvist_model": feat
"canvist_extensions": feat
---

# One crate per extension, combined by canvist_extensions

Each Rust extension is now its own crate under `crates/extensions`, named `canvist_extension_<name>`: `canvist_extension_bold`, `canvist_extension_heading`, `canvist_extension_table`, and so on. An app can depend on exactly the extensions it registers.

`canvist_extensions` keeps every feature and every public path: each feature now enables the matching crate and re-exports its types, so `canvist_extensions::Heading` and `canvist_extension_heading::Heading` are the same type. Nothing changes for code that uses `canvist_extensions` or `canvist::extensions`.

The crates follow TipTap's packages: one per mark and per media type, `canvist_extension_list` for every list type (as `@tiptap/extension-list`), and `canvist_extension_text_style` for the `textStyle` mark and its attributes (as `@tiptap/extension-text-style`).

The helpers the extensions shared are now public, for anyone writing their own:

- `canvist_core::query`: `around`, `attr_around`, `mark_active`, and `shared_mark_attr` read the selection's surroundings by type name.
- `canvist_core::SimpleMark` and `MarkHandle` describe a mark that only switches on and off, such as bold.
- `canvist_model::video_kind` is the policy for which URLs a video block may embed.
- `canvist_extension_media::MediaBlock` describes an atomic block a command inserts, such as an image.
- `canvist_extension_suggestion` finds trigger-character suggestions such as `@ad`, as TipTap's `@tiptap/suggestion` does.
