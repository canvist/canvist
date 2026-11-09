/**
 * canvist's document model and editing engine with no DOM, for Node, Deno,
 * Bun, and Workers: load and export ProseMirror JSON, HTML, Markdown, and
 * text, and run typed commands.
 *
 * This module compiles in TipTap's StarterKit. Every other extension is in
 * `@canvist/headless/full`, a larger module with the same API. Yjs
 * collaboration is in `@canvist/collab`, server-side rendering in
 * `@canvist/ssr`, and canvas scenes in `@canvist/scene`.
 *
 * ```ts
 * import { createEngine } from "@canvist/headless";
 *
 * const engine = await createEngine();
 * const doc = engine.parse("markdown", "Hello world");
 * doc.select({ path: [0], offset: 6 }, { path: [0], offset: 11 });
 * doc.commands.toggleBold();
 * doc.export("html"); // "<p>Hello <strong>world</strong></p>"
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
} from "@canvist/core";
export type * from "@canvist/core";
export * from "@canvist/core/extensions";
export { createEngine } from "./create.ts";
export { load, type WasmSource } from "./load.ts";
