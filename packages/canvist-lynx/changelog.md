All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.3.0](https://github.com/canvist/canvist/releases/tag/v0.3.0) (2026-11-10)

### 📝 Changed

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

## [0.2.1](https://github.com/canvist/canvist/releases/tag/v0.2.1) (2026-11-07)

### Changed

- **No package-specific changes were recorded; `@canvist/lynx` was updated to 0.2.1 as part of group `sdk`.**

## [0.2.0](https://github.com/canvist/canvist/releases/tag/v0.2.0) (2026-11-04)

### 🚀 Feature

#### A native rich-text editor for Lynx

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #198](https://github.com/canvist/canvist/pull/198)

`@canvist/lynx` puts the iOS and Android `CanvistView`s in Lynx apps as a `<canvist-editor>` element, with the same typed API as `@canvist/web` and `@canvist/react-native`: `CanvistEditor`, `useCanvistEditor`, and `useEditorState` in ReactLynx, and commands, queries, and documents typed from the extensions you register. Calls go through a synchronous `CanvistEditors` native module, which answers with a JSON envelope because Lynx reports native exceptions to the host rather than to JavaScript; failures still arrive as `CanvistError`s with their codes.

On iOS, the `CanvistLynx` pod registers the element and provides `CanvistLynxModule`; on Android, `CanvistLynx.behavior()` and `CanvistLynx.registerModule()` do the same. Both build on the `CanvistHostView` and `HostedEditors` that React Native uses.

`examples/lynx` is the notes screen from the React Native example in ReactLynx, with iOS and Android host apps; its Maestro flows run on a simulator and an emulator in CI. A call from JavaScript into the editor takes 50 to 400 µs at p95 on an iOS Simulator and 260 to 610 µs on an Android emulator (Release builds, averaged over batches, as Lynx's JavaScript engine has no high-resolution clock).
