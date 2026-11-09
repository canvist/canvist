/**
 * Where a WebAssembly module comes from, and reading the one a package
 * bundles. Loading none itself, it serves every canvist package that ships
 * a module.
 *
 * @module
 */

/**
 * Where the WebAssembly module comes from: its bytes, a compiled module (as
 * Cloudflare Workers import `.wasm` files), a URL, or a response.
 */
export type WasmSource =
	| ArrayBuffer
	| ArrayBufferView
	| WebAssembly.Module
	| URL
	| Response
	| Promise<Response>;

/**
 * A bundled module at `url`: read from disk when it is a file, fetched
 * otherwise.
 *
 * @internal
 */
export async function bundled(url: URL): Promise<WasmSource> {
	if (url.protocol !== "file:") {
		return url;
	}

	const [{ readFile }, { fileURLToPath }] = await Promise.all([
		import("node:fs/promises"),
		import("node:url"),
	]);
	// Pass strings: some type environments (`deno publish` with @types/node)
	// see the global `URL` and Node's `URL` as different types.
	return readFile(fileURLToPath(url.href));
}
