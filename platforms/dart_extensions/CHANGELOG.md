# Changelog

## [0.2.2](https://github.com/canvist/canvist/releases/tag/v0.2.2) (2026-11-20)

### 🚀 Feature

- **Markdown reveal: edit the Markdown of the formatting at the caret.** The new `markdownReveal` extension (in the full module) shows the Markdown of the formatting at the caret, as Typora and Obsidian do: `**` around bold text, a heading's `##`, a link's `[` and `](address)`, drawn faint and gone when the caret leaves. The arrow keys stop on both sides of the syntax, typing follows the side the caret is on, and Backspace or Delete over it deletes one of its characters: `**` becomes `*` (bold turns italic), and a heading's `##` becomes `#`. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #250](https://github.com/canvist/canvist/pull/250)

## 0.2.1

- Provide all built-in extension descriptors and typed handles in one package.
