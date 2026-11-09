/**
 * Two canvist peers catching up by state vector, and speaking y-websocket's
 * sync protocol to each other.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { createEngine } from "../src/mod.ts";

test("a peer sends what another is missing, by its state vector", async () => {
	using engine = await createEngine();
	using server = engine.parse("markdown", "Hello world");
	server.share(1);
	using peer = engine.fromYjs(server.encodeState(), 2);

	peer.selectAll();
	peer.commands.toggleBold();

	assert.equal(server.applyUpdate(peer.encodeDiff(server.stateVector())), true);
	assert.equal(server.export("html"), "<p><strong>Hello world</strong></p>");
});

test("peers greet, reply, and flush their changes as y-sync messages", async () => {
	using engine = await createEngine();
	using server = engine.parse("markdown", "Hello");
	server.share(1);
	using peer = engine.fromYjs(server.encodeState(), 2);

	for (const message of peer.receiveSync(server.syncStart()).reply) {
		server.receiveSync(message);
	}

	peer.select({ path: [0], offset: 5 });
	peer.insertText("!");
	const changes = peer.flushSync();

	assert.ok(changes.length > 0);
	assert.ok(changes.some((message) => server.receiveSync(message).changed));
	assert.equal(server.export("text"), "Hello!");
	assert.deepEqual(peer.flushSync(), []);
});
