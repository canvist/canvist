/**
 * canvist documents as Yjs peers, for Node, Deno, Bun, and Workers: join
 * shared state, merge updates from other peers, and serve y-websocket's sync
 * protocol, with the same typed commands as `@canvist/headless`.
 *
 * This module compiles in TipTap's StarterKit. Every other extension is in
 * `@canvist/collab/full`, a larger module with the same API.
 *
 * ```ts
 * import { createEngine } from "@canvist/collab";
 *
 * const engine = await createEngine();
 * using server = engine.parse("markdown", "Hello world");
 * server.share(1);
 * using peer = engine.fromYjs(server.encodeState(), 2);
 * peer.selectAll();
 * peer.commands.toggleBold();
 * peer.takeUpdates().forEach((update) => server.applyUpdate(update));
 * server.export("html"); // "<p><strong>Hello world</strong></p>"
 * ```
 *
 * @module
 */
export {
	type CanvistError,
	Doc,
	Engine,
	type EngineOptions,
	isCanvistError,
	type WasmSource,
} from "@canvist/core";
export type * from "@canvist/core";
export * from "@canvist/core/extensions";
export {
	CollabDoc,
	CollabEngine,
	type SyncReply,
	type Validation,
} from "./collab.ts";
export { createEngine } from "./create.ts";
export { load } from "./load.ts";
