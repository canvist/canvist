/**
 * Media blocks between Yjs peers: peers inserting media together, and a
 * JavaScript Yjs peer reading it.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import * as Y from "yjs";

import {
	audio,
	createEngine as createFullEngine,
	file,
	image,
	twitch,
	youtube,
} from "../src/full.ts";

const media = [image, youtube, twitch, audio, file] as const;

/** Block types and attributes, without block ids. */
function blocks(json: unknown): unknown {
	const content = (json as { content: { attrs?: Record<string, unknown> }[] })
		.content;

	return content.map(({ attrs, ...block }) => {
		const { id: _id, ...rest } = attrs ?? {};
		return Object.keys(rest).length > 0 ? { ...block, attrs: rest } : block;
	});
}

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

test("two peers inserting media concurrently converge", async () => {
	using firstEngine = await createFullEngine(media, { client: 1 });
	using secondEngine = await createFullEngine(media, { client: 2 });
	using first = firstEngine.parse("text", "one\ntwo");
	first.share(1);
	using second = secondEngine.fromYjs(first.encodeState(), 2);

	first.select({ path: [0], offset: 3 });
	first.commands.setImage({ src: "a.png", width: 320.5 });
	second.select({ path: [0], offset: 3 });
	second.commands.setAudio({ src: "a.mp3", loop: true });
	sync(first, second);

	assert.deepEqual(blocks(first.toJson()), blocks(second.toJson()));
	assert.equal(first.export("html"), second.export("html"));
	assert.match(first.export("html"), /<img src="a.png" width="320.5">/);
	assert.match(first.export("html"), /<audio src="a.mp3" controls loop>/);
	assert.deepEqual(second.validate(), { rejectedBlocks: 0, waiting: false });
});

test("a JavaScript Yjs peer reads and writes media blocks", async () => {
	using engine = await createFullEngine(media);
	using server = engine.create();
	server.share(1);
	server.commands.insertFile({
		url: "r.pdf",
		fileName: "Report",
		fileSize: 2048,
	});

	const peer = new Y.Doc();
	Y.applyUpdate(peer, server.encodeState());
	server.takeUpdates();
	const shared = peer.getArray<Y.Map<unknown>>("blocks");
	const card = shared.get(0);
	assert.equal(card?.get("type"), "file");
	assert.deepEqual((card?.get("attrs") as Y.Map<unknown>).toJSON(), {
		fileName: "Report",
		fileSize: 2048,
		url: "r.pdf",
	});

	const updates: Uint8Array[] = [];
	peer.on("update", (update: Uint8Array) => updates.push(update));
	const attrs = card?.get("attrs") as Y.Map<unknown>;
	attrs.set("fileName", "Renamed");
	updates.forEach((update) => assert.equal(server.applyUpdate(update), true));
	assert.match(server.export("html"), /data-file-name="Renamed"/);
});
