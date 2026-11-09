/**
 * Wire compatibility with JavaScript Yjs: a JavaScript Yjs peer reads and
 * edits a shared document.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import * as Y from "yjs";

import { createEngine } from "../src/mod.ts";

test("a JavaScript Yjs peer reads and edits a shared document", async () => {
	const engine = await createEngine();
	const server = engine.parse("markdown", "# Title\n\nBody");
	server.share(1);

	const peer = new Y.Doc();
	Y.applyUpdate(peer, server.encodeState());
	const blocks = peer.getArray<Y.Map<unknown>>("blocks");
	assert.equal(blocks.get(0)?.get("type"), "heading");
	assert.equal(String(blocks.get(1)?.get("text")), "Body");

	const updates: Uint8Array[] = [];
	peer.on("update", (update: Uint8Array) => updates.push(update));
	(blocks.get(1)?.get("text") as Y.Text).insert(4, " text", { bold: true });
	updates.forEach((update) => assert.equal(server.applyUpdate(update), true));
	assert.equal(server.export("markdown"), "# Title\n\nBody **text**");

	server.selectAll();
	server.commands.toggleItalic();
	server.takeUpdates().forEach((update) => Y.applyUpdate(peer, update));
	const formatted = (blocks.get(0)?.get("text") as Y.Text).toDelta() as {
		attributes?: Record<string, unknown>;
	}[];
	assert.deepEqual(formatted[0]?.attributes, { italic: true });
	assert.deepEqual(server.validate(), { rejectedBlocks: 0, waiting: false });
});
