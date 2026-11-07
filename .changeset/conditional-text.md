---
"canvist_core": feat
"canvist_extensions": feat
"canvist_layout": feat
"canvist_headless": feat
"@canvist/headless": feat
"@canvist/web": feat
"canvist-dart": feat
---

# Conditional text: words that only some versions of a document keep

The new `conditional` extension (in the full module) adds the `conditional` mark, for text that belongs in some versions of a document and not others: a sentence for one audience, or the handle a person goes by on one network when one post goes to several. Its `when` attribute is the host's own string, such as `"instagram threads"`; canvist never reads it, and the host decides what it means when it renders or exports a version.

- `setConditional({ when })`, `toggleConditional({ when })` and `unsetConditional` work on selected text; `insertConditional({ when, text })` writes conditional text at the caret.
- `isConditional` and `conditionalWhen` read the selection.
- The mark is not inclusive, so text typed after a conditional run is ordinary text, and the StarterKit theme tints conditional text.
- HTML writes and reads `<span data-type="conditional" data-when="…">`; Markdown keeps only the text.

`canvist_core::commands::insert_inline` is now public: it inserts text and embeds that carry their own marks over the selection.
