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

- **No package-specific changes were recorded; `@canvist/react-native` was updated to 0.2.1 as part of group `sdk`.**

## [0.2.0](https://github.com/canvist/canvist/releases/tag/v0.2.0) (2026-11-04)

### 🚀 Feature

#### A native rich-text editor for React Native

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #193](https://github.com/canvist/canvist/pull/193)

`@canvist/react-native` puts the iOS and Android `CanvistView`s in React Native apps as a Fabric component, not a WebView, so typing, input methods, autocorrect, dictation, selection handles, VoiceOver, and TalkBack work as they do in the native editors. Commands, queries, and documents are typed from the extensions you register, with the same API as `@canvist/web`, and calls go through a synchronous TurboModule, so a query answers at once:

```tsx
const { editor, props } = useCanvistEditor({ extensions: [bold, heading] });
const isBold = useEditorState(editor, (e) => e.query.isBold(), false);

<CanvistView {...props} style={{ flex: 1 }} />;
editor?.commands.toggleHeading({ level: 2 });
editor?.commands.toggleItalic(); // type error: italic is not registered
```

`CanvistEditor` wraps both in one component with `onReady` and `onUpdate`. A call from JavaScript into the editor takes 6 to 47 µs at p95 on an iOS Simulator and 210 to 344 µs on an Android emulator (Release builds); keystrokes never cross into JavaScript. The example app in `examples/react-native` is a notes screen with a formatting toolbar, and its Maestro flows run on a simulator and an emulator in CI.

- `@canvist/headless/native` is the typed editor over any host that runs canvist natively: a small synchronous bridge of names and JSON, with failures rethrown as `CanvistError`s. `@canvist/lynx` will share it.
- The Swift and Android libraries gain what JavaScript hosts share: `CanvistHostView`, a `CanvistView` started from an element's first props and reporting changes and focus; `HostedEditors`, a thread-safe registry of editors by id with failures as `[canvist:<code>] <reason>`; JSON ranges (`selectionJSON()`, `select(rangeJSON:)`); `CanvistView.onFocusChange`; and `ExtensionSet(names:)`.
- `platforms/apple/Canvist.podspec` builds the Swift package with CocoaPods.
- `canvist-android` compiles for the Kotlin 2.2 language and standard library, so apps on Kotlin 2.2, React Native 0.88's version, can use it.
