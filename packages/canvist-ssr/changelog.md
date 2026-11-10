All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.3.0](https://github.com/canvist/canvist/releases/tag/v0.3.0) (2026-11-10)

### 🚀 Feature

#### `@canvist/headless` splits into packages you pick from

_Owner:_ Ifiok Jr. · _Introduced in:_ [a3af64a](https://github.com/canvist/canvist/commit/a3af64a3b237ab7fd0717a4a274beb07ed7f4307)

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
