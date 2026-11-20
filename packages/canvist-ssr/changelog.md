All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.2.2](https://github.com/canvist/canvist/releases/tag/v0.2.2) (2026-11-20)

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

### 🐛 Fixed

#### Write the table spans the grid was built with to HTML

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #267](https://github.com/canvist/canvist/pull/267)

Server-rendered HTML wrote a cell's `colspan` and `rowspan` as the document held them, while the table's grid clamped them. A cell whose JSON claimed a `colspan` of `4294967295` was laid out across 1,000 columns but exported with the full number. The HTML now carries the clamped spans: at most 1,000 columns and 65,534 rows, the limits browsers apply.

`canvist_model` exports those limits as `MAX_COLSPAN` and `MAX_ROWSPAN`, and the table extension and the HTML reader use them in place of their own copies.

- **A WebAssembly module that failed to load can load again.** Each package loaded its module once and kept the first promise, failure included. After a dropped connection, or a first call without the module a Worker has to pass, every later `createEngine`, `createEditor`, `createRenderer`, and `createSceneEngine` failed the same way until the page or process restarted. A failed load is now forgotten, so the next call tries again with its own source. `@canvist/core` has the shared `loadOnce` the packages use. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #263](https://github.com/canvist/canvist/pull/263)
