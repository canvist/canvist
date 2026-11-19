/**
 * Loading the WebAssembly module, once per page: every editor shares one
 * instance, so creating a second editor costs no extra download or compile.
 *
 * @module
 */
import { loadOnce } from "@canvist/core";
import init from "../wasm/canvist_web.js";

/** Where the WebAssembly module comes from: its bytes, a compiled module, a URL, or a response. */
export type WasmSource =
	| ArrayBuffer
	| ArrayBufferView
	| WebAssembly.Module
	| URL
	| Response
	| Promise<Response>;

const loading = loadOnce<WasmSource>(async (source) => {
	await init({
		module_or_path: source ??
			new URL("../wasm/canvist_web_bg.wasm", import.meta.url),
	});
});

/**
 * Load the WebAssembly module once. Later calls share the first call's
 * promise, so passing a source only matters the first time, unless that
 * load failed (as when the connection dropped): then the next call tries
 * again. Without a source, the module is fetched from next to this file,
 * which bundlers such as Vite follow.
 */
export function load(source?: WasmSource): Promise<void> {
	return loading(source);
}
