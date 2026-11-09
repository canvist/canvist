/**
 * Mentions, hashtags, and emoji shared between Yjs peers: two canvist
 * documents, and a JavaScript Yjs peer, editing them together.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import * as Y from "yjs";

import { createEngine, emoji, hashtag, mention } from "../src/full.ts";

const atoms = [mention, hashtag, emoji] as const;

/** Deliver each peer's pending updates to the other. */
function sync(
	first: {
		takeUpdates(): Uint8Array[];
		applyUpdate(update: Uint8Array): boolean;
	},
	second: {
		takeUpdates(): Uint8Array[];
		applyUpdate(update: Uint8Array): boolean;
	},
): void {
	const fromFirst = first.takeUpdates();
	const fromSecond = second.takeUpdates();
	fromFirst.forEach((update) => second.applyUpdate(update));
	fromSecond.forEach((update) => first.applyUpdate(update));
}

test("two peers adding atoms concurrently converge", async () => {
	using firstEngine = await createEngine(atoms, { client: 1 });
	using secondEngine = await createEngine(atoms, { client: 2 });
	using first = firstEngine.parse("text", "Hello world");
	first.share(1);
	using second = secondEngine.fromYjs(first.encodeState(), 2);

	first.select({ path: [0], offset: 5 });
	first.typeText(" @gr");
	first.commands.insertMention({ id: "3", label: "Grace" });
	second.select({ path: [0], offset: 5 });
	second.typeText(" :rocket: #wasm ");
	sync(first, second);

	const strip = (json: unknown) =>
		JSON.stringify((json as { content: unknown }).content);
	assert.equal(strip(first.toJson()), strip(second.toJson()));
	assert.equal(first.export("html"), second.export("html"));
	assert.match(first.export("html"), /data-label="Grace"/);
	assert.match(first.export("html"), /data-name="rocket"/);
	assert.deepEqual(first.query.hashtags(), ["wasm"]);
	assert.deepEqual(second.validate(), { rejectedBlocks: 0, waiting: false });

	assert.equal(first.undo(), true);
	sync(first, second);
	assert.doesNotMatch(second.export("html"), /data-type="mention"/);
	assert.equal(first.export("html"), second.export("html"));
});

test("a JavaScript Yjs peer reads and writes atoms and hashtags", async () => {
	using engine = await createEngine(atoms);
	using server = engine.create();
	server.share(1);
	server.typeText("@a");
	server.commands.insertMention({ id: "7", label: "Ada" });
	server.typeText(":wave: #team ");

	const peer = new Y.Doc();
	Y.applyUpdate(peer, server.encodeState());
	server.takeUpdates();
	const text = peer.getArray<Y.Map<unknown>>("blocks").get(0)?.get(
		"text",
	) as Y.Text;
	const delta = text.toDelta() as {
		insert: unknown;
		attributes?: Record<string, unknown>;
	}[];
	assert.deepEqual(delta[0]?.insert, {
		mention: { id: "7", label: "Ada", mentionSuggestionChar: "@" },
	});
	assert.deepEqual(delta[2]?.insert, { emoji: { name: "wave", emoji: "👋" } });
	assert.deepEqual(delta[4], {
		insert: "#team",
		attributes: { hashtag: { tag: "team" } },
	});

	const updates: Uint8Array[] = [];
	peer.on("update", (update: Uint8Array) => updates.push(update));
	const end = text.length;
	text.insertEmbed(end, {
		mention: { id: "9", label: "Zed", mentionSuggestionChar: "@" },
	});
	text.insert(end + 1, " #js", {});
	text.format(end + 2, 3, { hashtag: { tag: "js" } });
	updates.forEach((update) => assert.equal(server.applyUpdate(update), true));

	assert.equal(server.export("text"), "@Ada 👋 #team @Zed #js");
	assert.deepEqual(server.query.hashtags(), ["js", "team"]);
	assert.deepEqual(server.validate(), { rejectedBlocks: 0, waiting: false });
});
