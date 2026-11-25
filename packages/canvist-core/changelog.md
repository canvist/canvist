All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.2.2](https://github.com/canvist/canvist/releases/tag/v0.2.2) (2026-11-25)

### 🚀 Feature

#### `@canvist/headless` splits into packages you pick from

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #253](https://github.com/canvist/canvist/pull/253)

`@canvist/headless` carried every feature: the StarterKit and full modules with Yjs compiled in, server-side rendering, and canvas scenes, about 10 MB of WebAssembly in one install. Each now has its own package, and the typed half moves to one with no WebAssembly at all:

| Package             | What it has                                                                       |
| ------------------- | --------------------------------------------------------------------------------- |
| `@canvist/core`     | `Engine`, `Doc`, errors, types, and every extension's descriptor. No WebAssembly. |
| `@canvist/headless` | The StarterKit module, and `/full` with every extension. No Yjs.                  |
| `@canvist/collab`   | The same API with Yjs: `fromYjs`, `share`, `applyUpdate`, y-sync, and `/full`.    |
| `@canvist/ssr`      | Server-side rendering, formerly `@canvist/headless/ssr`, now without Yjs.         |
| `@canvist/scene`    | Canvas scenes, formerly `@canvist/headless/scene`.                                |

Without Yjs, the StarterKit module is 634 KB instead of 1,119 KB (267 KB gzipped instead of 457 KB), the full module 964 KB instead of 1,429 KB, and the renderer 1,826 KB instead of 2,306 KB. The collaborating modules are the size the old ones were.

Every `@canvist/extension-*` package, `@canvist/web`, `@canvist/canvas`, `@canvist/react-native`, and `@canvist/lynx` now depend on `@canvist/core` instead of `@canvist/headless`, so picking an extension or an editor installs no engine module.

##### Migrating

- Documents that collaborate: import `createEngine` from `@canvist/collab` (or `@canvist/collab/full`) instead of `@canvist/headless`. Its engines are `CollabEngine`s and their documents `CollabDoc`s, with the same methods the old ones had. `SyncReply` and `Validation` come from there too.
- `@canvist/headless/ssr` is `@canvist/ssr`, and `@canvist/headless/scene` is `@canvist/scene`, with the same exports. Scene JSON types are also in `@canvist/core/scene`.
- `@canvist/headless/extensions`, `/extras`, and `/native` still work; new code can import them from `@canvist/core`.

- **Markdown reveal: edit the Markdown of the formatting at the caret.** The new `markdownReveal` extension (in the full module) shows the Markdown of the formatting at the caret, as Typora and Obsidian do: `**` around bold text, a heading's `##`, a link's `[` and `](address)`, drawn faint and gone when the caret leaves. The arrow keys stop on both sides of the syntax, typing follows the side the caret is on, and Backspace or Delete over it deletes one of its characters: `**` becomes `*` (bold turns italic), and a heading's `##` becomes `#`. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #250](https://github.com/canvist/canvist/pull/250)

### 🐛 Fixed

- **A WebAssembly module that failed to load can load again.** Each package loaded its module once and kept the first promise, failure included. After a dropped connection, or a first call without the module a Worker has to pass, every later `createEngine`, `createEditor`, `createRenderer`, and `createSceneEngine` failed the same way until the page or process restarted. A failed load is now forgotten, so the next call tries again with its own source. `@canvist/core` has the shared `loadOnce` the packages use. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #263](https://github.com/canvist/canvist/pull/263)
