/**
 * The typed half of canvist for JavaScript: the engine and document classes,
 * every extension's descriptor, and the types that make commands and queries
 * of unregistered extensions a type error. It loads no WebAssembly, so
 * extension packages and editors depend on it without pulling a module in;
 * `@canvist/headless`, `@canvist/collab`, and `@canvist/web` bring one.
 *
 * ```ts
 * import { bold, heading } from "@canvist/core/extensions";
 * import { createEngine } from "@canvist/headless";
 *
 * const engine = await createEngine([bold, heading]);
 * ```
 *
 * @module
 */
export { bundled, type WasmSource } from "./bundled.ts";
export { Doc, type DocWrapper, Engine, type EngineOptions } from "./engine.ts";
export { type CanvistError, isCanvistError } from "./errors.ts";
export * from "./extensions.generated.ts";
export type * from "./types.ts";
export type { WasmDoc, WasmEngine } from "./wasm.ts";
