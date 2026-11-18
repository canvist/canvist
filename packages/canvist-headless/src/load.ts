/**
 * Loading the WebAssembly module in any JavaScript runtime.
 *
 * @module
 */
import { bundled, loadOnce, type WasmSource } from "@canvist/core";
import init from "../wasm/canvist_headless_wasm.js";

export type { WasmSource } from "@canvist/core";

const BUNDLED = new URL(
	"../wasm/canvist_headless_wasm_bg.wasm",
	import.meta.url,
);

const loading = loadOnce(instantiate);

/**
 * Load the WebAssembly module once. Later calls share the first call's
 * promise, so passing a source only matters the first time, unless that
 * load failed: then the next call tries again.
 *
 * Without a source, Node, Deno, and Bun read the bundled file from disk.
 * Runtimes without a file system, such as Workers, pass the module they
 * imported: `await load(wasmModule)`.
 */
export function load(source?: WasmSource): Promise<void> {
	return loading(source);
}

async function instantiate(source: WasmSource | undefined): Promise<void> {
	await init({ module_or_path: source ?? await bundled(BUNDLED) });
}
