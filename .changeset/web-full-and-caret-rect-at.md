---
"@canvist/web": feat
"@canvist/headless": feat
canvist_web: feat
---

# Edit every extension in the browser, and draw collaborators' carets

- `createEditor` from `@canvist/web/full` takes every extension canvist ships, not only the StarterKit: tables, callouts, math, media, ink, and the rest, laid out and painted on the canvas. It loads a third WebAssembly module (about 470 KB gzipped, against 340 KB for the StarterKit build), so StarterKit editors never download it. The entry point also exports everything `@canvist/web` does, every extension, and `allExtensions`.
- `editor.caretRectAt(position)` is where a caret at `position` is drawn, in document coordinates, or `null` when the document has no such position or it isn't laid out. Hosts draw other people's carets with it; the collaboration guide shows how.
- `@canvist/headless/extras` exports the extensions beyond the StarterKit and `allExtensions` on their own, without a WebAssembly module.
- `canvist_web` has a `full` feature: every extension, the emoji table, and the layout for tables, ink, kinetic text, and the theme's blocks and marks beyond the StarterKit.
