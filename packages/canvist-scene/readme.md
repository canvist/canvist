<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/quick-start/canvas.md"><strong>Quick start</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/@canvist/scene"><img src="https://img.shields.io/npm/v/%40canvist%2Fscene?color=3B2CF5" alt="npm version" /></a>
  <a href="https://github.com/canvist/canvist/actions/workflows/headless-js.yml"><img src="https://github.com/canvist/canvist/actions/workflows/headless-js.yml/badge.svg?branch=main" alt="headless-js status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# @canvist/scene

canvist canvas scenes on a server, for Node, Deno, Bun, and Workers: load scene JSON or Yjs state, run canvas commands and inspector edits, and export SVG and PNG with the renderer [`@canvist/canvas`](../canvist-canvas) draws with. Use it for thumbnails, social cards, and designs an agent writes.

```sh
npm install @canvist/scene
```

```ts
import { createSceneEngine } from "@canvist/scene";

const engine = await createSceneEngine();
using doc = engine.fromJson({
	type: "canvist-scene",
	version: 1,
	nodes: [{
		type: "page",
		children: [{
			type: "rectangle",
			props: { size: [120, 80], fills: [{ type: "solid", color: "#3b2cf5" }] },
		}],
	}],
});
const png = doc.exportPng({ scale: 2 });
```

- **Scenes:** `engine.create()` starts from one empty page, `engine.fromJson(scene)` reads scene JSON, nested or flat, and `doc.toJson()` returns the canonical flat form.
- **Edits:** `run(name, args)` runs a canvas command, `select(ids)` and `selection()` set and read the selection, `inspect()` and `setField(key, value)` read and edit it as the inspector does, and `undo()` reverts the latest change. `engine.metadata()` lists the tools, commands, node types, and inspector sections.
- **Export:** `exportSvg(options)` and `exportPng(options)` export the scene, or the nodes `options` names. `addImage(asset, width, height, rgba)` provides an image's pixels; without them, an image is drawn as a grey placeholder.
- **Collaboration:** `share()`, `encodeState()`, `applyUpdate(update)`, and `takeUpdates()`, and `engine.fromYjs(state)` joins a peer's scene.
- **Workers:** runtimes without a file system pass the module they imported: `createSceneEngine({ wasm })`, with `wasm` from `@canvist/scene/wasm/canvist_scene_bg.wasm`.

The scene types (`SceneJson`, `ExportOptions`, `NodeId`, …) are exported here, and from `@canvist/core/scene` for code that only needs the types. Scenes live in their own WebAssembly module, so document packages such as `@canvist/headless` stay small.

<details>
<summary><strong>Development</strong></summary>

```sh
(cd ../canvist-core && npm ci && npm run build)   # the package this one builds on
npm ci
npm run build:wasm      # the scene module into wasm/
npm test                # tests in Node
deno task test          # the same, in Deno
npm run check:types     # tsc --noEmit
npm run build           # JavaScript and declarations for npm, into dist/
```

</details>
