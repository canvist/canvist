<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/quick-start/server-typescript.md"><strong>Quick start</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/@canvist/core"><img src="https://img.shields.io/npm/v/%40canvist%2Fcore?color=3B2CF5" alt="npm version" /></a>
  <a href="https://github.com/canvist/canvist/actions/workflows/headless-js.yml"><img src="https://github.com/canvist/canvist/actions/workflows/headless-js.yml/badge.svg?branch=main" alt="headless-js status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# @canvist/core

The typed half of canvist for JavaScript: the engine and document classes, every extension's descriptor, and the types that make commands and queries of unregistered extensions a type error. It loads no WebAssembly, so the extension packages and the editors depend on it without pulling a module in.

You rarely install it yourself. [`@canvist/headless`](../canvist-headless), [`@canvist/collab`](../canvist-collab), [`@canvist/ssr`](../canvist-ssr), [`@canvist/scene`](../canvist-scene), and [`@canvist/web`](../canvist-web) bring a module and re-export what you need from here. Import from it directly in code that only needs the types or descriptors, such as a library that takes a document from its caller.

```sh
npm install @canvist/core
```

```ts
import { bold, heading } from "@canvist/core/extensions";
import { createEngine } from "@canvist/headless";

const engine = await createEngine([bold, heading]);
const doc = engine.create();

doc.commands.setHeading({ level: 2 }); // levels are 1 | 2 | 3 | 4 | 5 | 6
doc.commands.toggleItalic(); // type error: italic is not registered
```

## Entry points

| Import                     | What it has                                                                                                                                                                                          |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `@canvist/core`            | `Engine`, `Doc`, `isCanvistError`, `CanvistError`, `EngineOptions`, `WasmSource`, the StarterKit's descriptors, and every type (`StarterKit`, `Extensions`, `DocJson`, `Content`, `ThemeOptions`, …) |
| `@canvist/core/extensions` | The StarterKit's descriptors, and `core`, the four every engine has                                                                                                                                  |
| `@canvist/core/extras`     | Every other extension's descriptor, `extras`, and `allExtensions`                                                                                                                                    |
| `@canvist/core/native`     | `NativeEditor` and `NativeBridge`, the typed editor `@canvist/react-native` and `@canvist/lynx` put over the native views                                                                            |
| `@canvist/core/scene`      | The scene JSON types: `SceneJson`, `ExportOptions`, `NodeId`, …                                                                                                                                      |
| `@canvist/core/gemoji`     | GitHub's emoji table, which the full modules load for `emoji`                                                                                                                                        |

Errors are `CanvistError`s with the same stable `code`s as the Rust and Dart packages. Positions are `{ path, offset }`, where `path` walks the block tree and `offset` counts UTF-16 code units, like JavaScript strings.

<details>
<summary><strong>Development</strong></summary>

```sh
npm ci
npm run generate          # regenerate the extension descriptors from the Rust metadata
npm run check:generated   # fail if they are out of date
npm run check:types       # tsc --noEmit
npm test                  # unit tests in Node
deno task test            # the same, in Deno
npm run build             # JavaScript and declarations for npm, into dist/
```

`tests/conformance.ts` runs the shared fixtures in [`conformance/`](../../conformance) against an engine module. `@canvist/headless` and `@canvist/collab` drive it from their own tests.

</details>
