/**
 * The document tools through `@canvist/collab/full`: comments shared with a
 * JavaScript Yjs peer.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import * as Y from "yjs";

import { comments, createEngine, starterKit } from "../src/full.ts";

test("a JavaScript Yjs peer sees comments as formatting and adds its own", async () => {
	using engine = await createEngine([...starterKit, comments]);
	using server = engine.parse("text", "Shared words");
	server.share(1);
	server.select({ path: [0], offset: 0 }, { path: [0], offset: 6 });
	server.commands.setComment({ threadId: "a" });

	const peer = new Y.Doc();
	Y.applyUpdate(peer, server.encodeState());
	server.takeUpdates();
	const text = peer.getArray<Y.Map<unknown>>("blocks").get(0)?.get(
		"text",
	) as Y.Text;
	const delta = text.toDelta() as {
		insert: string;
		attributes?: Record<string, unknown>;
	}[];
	assert.deepEqual(delta[0], {
		insert: "Shared",
		attributes: { comment: { threads: ["a"] } },
	});

	const updates: Uint8Array[] = [];
	peer.on("update", (update: Uint8Array) => updates.push(update));
	text.format(7, 5, { comment: { threads: ["b"] } });
	updates.forEach((update) => assert.equal(server.applyUpdate(update), true));

	assert.deepEqual(server.query.comments(), [
		{ threadId: "a", text: "Shared" },
		{ threadId: "b", text: "words" },
	]);
	assert.deepEqual(server.validate(), { rejectedBlocks: 0, waiting: false });
});
