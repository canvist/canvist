/**
 * Loading the collaborating modules from compiled `WebAssembly.Module`s,
 * as Cloudflare Workers do when they import `.wasm` files. Under
 * `node --test` each file runs in its own process, so these are the first
 * loads.
 *
 * @module
 */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";

import { createEngine as createFullEngine } from "../src/full.ts";
import { createEngine } from "../src/mod.ts";

async function compiled(path: string): Promise<WebAssembly.Module> {
	return WebAssembly.compile(await readFile(new URL(path, import.meta.url)));
}

test("loads from compiled modules, as Workers do", async () => {
	const wasm = await compiled("../wasm/canvist_collab_bg.wasm");
	using engine = await createEngine(undefined, { wasm });
	using doc = engine.parse("markdown", "**ok**");
	doc.share(1);
	assert.equal(doc.export("html"), "<p><strong>ok</strong></p>");

	const full = await compiled("../wasm-full/canvist_collab_full_bg.wasm");
	using fullEngine = await createFullEngine(undefined, { wasm: full });
	assert.equal(fullEngine.create().export("text"), "");
});
