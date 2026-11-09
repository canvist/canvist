/**
 * Ink and kinetic text through `@canvist/collab/full`: drawings and kinetic
 * text shared with a JavaScript Yjs peer.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import * as Y from "yjs";

import { createEngine, ink, kinetic, paragraph } from "../src/full.ts";

test("a JavaScript Yjs peer sees strokes and kinetic text and adds its own", async () => {
	using engine = await createEngine([paragraph, ink, kinetic]);
	using server = engine.fromJson({
		type: "doc",
		content: [
			{ type: "ink" },
			{ type: "paragraph", content: [{ type: "text", text: "Say hello" }] },
		],
	});
	server.share(1);
	server.commands.addInkStroke({ ink: 0, points: [1, 2, 3, 4] });

	const peer = new Y.Doc();
	Y.applyUpdate(peer, server.encodeState());
	server.takeUpdates();
	const blocks = peer.getArray<Y.Map<unknown>>("blocks");
	assert.equal(blocks.get(0)?.get("type"), "ink");

	const updates: Uint8Array[] = [];
	peer.on("update", (update: Uint8Array) => updates.push(update));
	(blocks.get(1)?.get("text") as Y.Text).format(4, 5, {
		kinetic: { effect: "rainbow" },
	});
	updates.forEach((update) => assert.equal(server.applyUpdate(update), true));

	server.select({ path: [1], offset: 5 }, { path: [1], offset: 6 });
	assert.equal(server.query.kineticEffect(), "rainbow");
	assert.deepEqual(server.query.inks(), [{ height: 400, strokes: 1 }]);
	assert.deepEqual(server.validate(), { rejectedBlocks: 0, waiting: false });
});
