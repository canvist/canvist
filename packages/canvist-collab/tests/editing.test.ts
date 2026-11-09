/**
 * The editing helpers beyond the StarterKit between Yjs peers: alignment
 * that JavaScript Yjs peers read and converge on.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import * as Y from "yjs";

import { createEngine, starterKit, textAlign } from "../src/full.ts";

test("JavaScript Yjs peers read alignment and converge on it", async () => {
	using engine = await createEngine([...starterKit, textAlign]);
	using server = engine.parse("markdown", "# Title\n\nBody");
	server.share(1);
	using other = engine.fromYjs(server.encodeState(), 2);

	const peer = new Y.Doc();
	Y.applyUpdate(peer, server.encodeState());
	const blocks = peer.getArray<Y.Map<unknown>>("blocks");

	server.select({ path: [0], offset: 0 });
	server.commands.setTextAlign({ alignment: "center" });
	other.select({ path: [1], offset: 0 });
	other.commands.setTextAlign({ alignment: "right" });

	const fromServer = server.takeUpdates();
	const fromOther = other.takeUpdates();
	fromServer.forEach((update) => {
		other.applyUpdate(update);
		Y.applyUpdate(peer, update);
	});
	fromOther.forEach((update) => {
		server.applyUpdate(update);
		Y.applyUpdate(peer, update);
	});

	assert.deepEqual(server.toJson(), other.toJson());
	assert.equal(
		server.export("html"),
		'<h1 style="text-align: center">Title</h1><p style="text-align: right">Body</p>',
	);
	// Block attributes live in each block's `attrs` Y.Map.
	const alignment = (index: number) =>
		(blocks.get(index)?.get("attrs") as Y.Map<unknown> | undefined)?.get(
			"textAlign",
		);
	assert.equal(alignment(0), "center");
	assert.equal(alignment(1), "right");
});
