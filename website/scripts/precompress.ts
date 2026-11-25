/**
 * Writes a Brotli (`.br`) and a gzip (`.gz`) copy beside every compressible
 * file in `dist/`, so Caddy serves them as they are (`file_server
 * precompressed`) instead of compressing each response on the fly.
 *
 *   deno run --allow-read --allow-write scripts/precompress.ts [dir]
 *
 * On-the-fly compression has to be fast, so Caddy's default zstd level
 * trades size for speed. Compressing once at build time can use the
 * slowest, smallest settings: Brotli at quality 11 with the largest window
 * served the editor's WebAssembly in 273 KB against 346 KB of Caddy's zstd,
 * and scripts about 20% smaller. Every browser that sends `zstd` also sends
 * `br`, so a zstd copy would never be chosen; gzip stays for the rest.
 *
 * A copy is kept only when it saves at least 5% and 64 bytes; otherwise the
 * file is served as it is.
 *
 * @module
 */
import { walk } from "@std/fs/walk";
import { Buffer } from "node:buffer";
import { brotliCompressSync, constants, gzipSync } from "node:zlib";

const root = Deno.args[0] ?? new URL("../dist", import.meta.url).pathname;

/** Text and WebAssembly; images, fonts, and video are compressed already. */
const COMPRESSIBLE =
	/\.(html|js|mjs|css|json|wasm|svg|txt|xml|riv|webmanifest)$/;

function worthIt(raw: number, compressed: number): boolean {
	return compressed <= raw * 0.95 && raw - compressed >= 64;
}

let files = 0;
let rawTotal = 0;
let brotliTotal = 0;

for await (
	const entry of walk(root, { includeDirs: false, match: [COMPRESSIBLE] })
) {
	const source = Buffer.from(await Deno.readFile(entry.path));
	const brotli = brotliCompressSync(source, {
		params: {
			[constants.BROTLI_PARAM_QUALITY]: constants.BROTLI_MAX_QUALITY,
			// 24 bits, the largest window browsers decode.
			[constants.BROTLI_PARAM_LGWIN]: constants.BROTLI_MAX_WINDOW_BITS,
			[constants.BROTLI_PARAM_SIZE_HINT]: source.byteLength,
			[constants.BROTLI_PARAM_MODE]:
				entry.path.endsWith(".wasm") || entry.path.endsWith(".riv")
					? constants.BROTLI_MODE_GENERIC
					: constants.BROTLI_MODE_TEXT,
		},
	});
	const gzip = gzipSync(source, { level: 9 });
	if (worthIt(source.byteLength, brotli.byteLength)) {
		await Deno.writeFile(`${entry.path}.br`, brotli);
	}
	if (worthIt(source.byteLength, gzip.byteLength)) {
		await Deno.writeFile(`${entry.path}.gz`, gzip);
	}
	files++;
	rawTotal += source.byteLength;
	brotliTotal += Math.min(source.byteLength, brotli.byteLength);
}

console.log(
	`precompressed ${files} files: ${(rawTotal / 1024).toFixed(0)} KiB → ${
		(brotliTotal / 1024).toFixed(0)
	} KiB with Brotli`,
);
