<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/guides/collaboration.md"><strong>Quick start</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/@canvist/collab"><img src="https://img.shields.io/npm/v/%40canvist%2Fcollab?color=3B2CF5" alt="npm version" /></a>
  <a href="https://github.com/canvist/canvist/actions/workflows/headless-js.yml"><img src="https://github.com/canvist/canvist/actions/workflows/headless-js.yml/badge.svg?branch=main" alt="headless-js status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# @canvist/collab

canvist documents as Yjs peers, for Node, Deno, Bun, and Workers: join shared state, merge updates from other peers, and serve y-websocket's sync protocol, with the same typed commands as [`@canvist/headless`](../canvist-headless). Browser editors in the same room join with `@canvist/web/collab`.

```sh
npm install @canvist/collab
```

```ts
import { createEngine } from "@canvist/collab";

const engine = await createEngine();
using server = engine.parse("markdown", "Hello world");
server.share(1);
using peer = engine.fromYjs(server.encodeState(), 2);
peer.selectAll();
peer.commands.toggleBold();
peer.takeUpdates().forEach((update) => server.applyUpdate(update));
server.export("html"); // "<p><strong>Hello world</strong></p>"
```

`createEngine` returns a `CollabEngine`, whose documents are `CollabDoc`s: everything a `@canvist/headless` document does, plus these calls.

- **Peers:** `share(clientId)`, `encodeState()`, `stateVector()`, `encodeDiff(stateVector)`, `takeUpdates()`, and `applyUpdate(update)`. `engine.fromYjs(state, clientId)` joins from a state.
- **Sync server:** `syncStart()` greets a connection, `receiveSync(message)` returns `{ reply, broadcast, changed }`, and `flushSync()` sends the server's own changes. [`examples/server`](./examples/server) serves `POST /convert` and a `/sync/:room` WebSocket in a few lines, and is tested against real Yjs clients.
- **Checks:** `validate()` reports shared blocks the schema rejected, and updates still waiting for earlier ones.

Yjs adds about 190 KB, gzipped, to the module, so use `@canvist/headless` for documents that don't collaborate. A collaborator's keystroke costs about 4.6 µs in Node at 10,000 paragraphs; `node bench/overhead.ts` reproduces it.

## Every other extension

This module compiles in TipTap's StarterKit. Every other extension is in `@canvist/collab/full`, a larger module with the same API:

```ts
import { createEngine, highlight, starterKit } from "@canvist/collab/full";

const engine = await createEngine([...starterKit, highlight]);
using doc = engine.parse("markdown", "Hello world");
doc.share(1);
doc.selectAll();
doc.commands.toggleHighlight();
doc.takeUpdates(); // the change, for the other peers
```

## Workers

Runtimes without a file system pass the module they imported:

```ts
import { createEngine } from "@canvist/collab";
import wasm from "@canvist/collab/wasm/canvist_collab_bg.wasm";

const engine = await createEngine(undefined, { wasm });
```

See [Collaboration](../../docs/src/guides/collaboration.md) for rooms, sync servers, and canvist Cloud Sync.

<details>
<summary><strong>Development</strong></summary>

```sh
(cd ../canvist-core && npm ci && npm run build)   # the package this one builds on
npm ci
npm run build:wasm      # the StarterKit module into wasm/, and the full one into wasm-full/, with the collab feature
npm test                # conformance, API, and sync tests in Node
deno task test          # the same, plus the server example, in Deno
deno task server        # run the example server
npm run check:types     # tsc --noEmit
npm run build           # JavaScript and declarations for npm, into dist/
```

The tests run every case in the shared fixtures in [`conformance/`](../../conformance), the collaboration ones too.

</details>
