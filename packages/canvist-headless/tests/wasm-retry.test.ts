/**
 * A load that fails is not final: the next call loads again, as when a
 * Worker's first call passes no module and a later one passes the module it
 * imported. Under `node --test` each file runs in its own process, so the
 * failing call is the first load.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { createEngine } from "../src/mod.ts";

test("a failed load is tried again by the next call", async () => {
	// The magic number with no version after it: not a module.
	const broken = new Uint8Array([0x00, 0x61, 0x73, 0x6d]);
	await assert.rejects(createEngine(undefined, { wasm: broken }));

	const engine = await createEngine();
	assert.equal(
		engine.parse("markdown", "**ok**").export("html"),
		"<p><strong>ok</strong></p>",
	);
});
