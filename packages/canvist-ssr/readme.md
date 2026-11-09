<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/quick-start/ssr.md"><strong>Quick start</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/@canvist/ssr"><img src="https://img.shields.io/npm/v/%40canvist%2Fssr?color=3B2CF5" alt="npm version" /></a>
  <a href="https://github.com/canvist/canvist/actions/workflows/headless-js.yml"><img src="https://github.com/canvist/canvist/actions/workflows/headless-js.yml/badge.svg?branch=main" alt="headless-js status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# @canvist/ssr

Server-side rendering for canvist documents, for Node, Deno, Bun, and Workers: semantic HTML and a stylesheet laid out exactly as [`@canvist/web`](../canvist-web) paints the document, so a page shows the text before any JavaScript runs and the live editor takes its place without moving a pixel. SVG images of the whole document, for previews and thumbnails, come from the same layout.

```sh
npm install @canvist/ssr
```

```ts
import { createRenderer } from "@canvist/ssr";

// Keep one renderer per server: it caches text measurements.
const renderer = await createRenderer({ theme: { scheme: "dark" } });
const { html, css } = renderer.html(
	{ format: "markdown", input: "# Release notes" },
	{ width: 640 },
);
```

- **Content** is text in a format (`markdown`, `html`, `text`, or `json`) or ProseMirror JSON, such as a stored `doc.toJson()`.
- **Match the editor:** pass the browser editor's `theme` and `extensions` to `createRenderer`, and render at the width of the element the editor hydrates in.
- **Pages and images:** `renderer.page(content, { width })` returns a complete HTML page, and `renderer.svg(content, { width })` an SVG image.
- **Fonts:** HTML needs none, because the browser lays it out. SVG text is measured with the renderer's own copy of each font, never the system's: the bundled Atkinson Hyperlegible stands in for every family until you pass yours as `fonts` or call `renderer.registerFont(bytes)`.
- **Workers:** runtimes without a file system pass the module they imported as `wasm`, from `@canvist/ssr/wasm/canvist_ssr_bg.wasm`.

Rendering lives in its own WebAssembly module, so document packages such as `@canvist/headless` stay small. The Rust `canvist` crate renders the same HTML with its `ssr` feature. See [Server-side rendering](../../docs/src/quick-start/ssr.md) for hydrating, Next.js, and Nuxt.

<details>
<summary><strong>Development</strong></summary>

```sh
(cd ../canvist-core && npm ci && npm run build)                          # the package this one builds on
(cd ../canvist-headless && npm ci && npm run build:wasm && npm run build) # the tests parse documents with it
npm ci
npm run build:wasm      # the rendering module into wasm/
npm test                # tests in Node
deno task test          # the same, in Deno
npm run check:types     # tsc --noEmit
npm run build           # JavaScript and declarations for npm, into dist/
```

</details>
