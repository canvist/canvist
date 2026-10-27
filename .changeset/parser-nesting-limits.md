---
sdk:
  bump: minor
  type: security
---

# Bound nesting in Markdown and HTML imports

Markdown blocks, inline syntax, and embedded HTML now share a maximum of 32 active parser levels. HTML containers and marks consume the same budget before nested trees are built, bounding recursive normalization and disposal. This prevents deeply nested imported or pasted markup from exhausting the native or WebAssembly stack.

`clipboard::try_from_html` and `clipboard::try_from_markdown` return `NestingLimitExceeded` without returning partial content. Headless conversion, import, and paste, web HTML paste and drop, and native HTML/Markdown paste propagate a normal `parse` error before editing the document. The existing infallible Rust readers preserve over-limit input as literal plain text.
