/**
 * The formatting extensions beyond the StarterKit between Yjs peers: marks a
 * JavaScript peer reads and writes, and canvist peers formatting
 * concurrently.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import * as Y from "yjs";

import {
	backgroundColor,
	color,
	createEngine,
	fontFamily,
	fontSize,
	highlight,
	lineHeight,
	starterKit,
	subscript,
	superscript,
	textStyle,
} from "../src/full.ts";

const formatting = [
	subscript,
	superscript,
	highlight,
	textStyle,
	color,
	backgroundColor,
	fontFamily,
	fontSize,
	lineHeight,
] as const;

test("a JavaScript Yjs peer reads and writes formatting marks", async () => {
	using engine = await createEngine([...starterKit, ...formatting]);
	using server = engine.parse("text", "Hello world");
	server.share(1);

	const peer = new Y.Doc();
	Y.applyUpdate(peer, server.encodeState());
	const blocks = peer.getArray<Y.Map<unknown>>("blocks");
	const text = blocks.get(0)?.get("text") as Y.Text;

	server.select({ path: [0], offset: 0 }, { path: [0], offset: 5 });
	server.commands.setHighlight({ color: "#fef08a" });
	server.commands.setFontSize({ fontSize: "20px" });
	server.takeUpdates().forEach((update) => Y.applyUpdate(peer, update));

	const delta = text.toDelta() as {
		insert: string;
		attributes?: Record<string, unknown>;
	}[];
	assert.deepEqual(delta[0], {
		insert: "Hello",
		attributes: {
			highlight: { color: "#fef08a" },
			textStyle: { fontSize: "20px" },
		},
	});

	const updates: Uint8Array[] = [];
	peer.on("update", (update: Uint8Array) => updates.push(update));
	text.format(6, 5, { superscript: true, textStyle: { color: "#e8590c" } });
	updates.forEach((update) => assert.equal(server.applyUpdate(update), true));

	assert.equal(
		server.export("html"),
		'<p><span style="font-size: 20px"><mark data-color="#fef08a" style="background-color: #fef08a; color: inherit">Hello</mark></span> <span style="color: #e8590c"><sup>world</sup></span></p>',
	);
	assert.deepEqual(server.validate(), { rejectedBlocks: 0, waiting: false });
});

test("two TypeScript peers formatting concurrently converge", async () => {
	using engine = await createEngine([...starterKit, ...formatting]);
	using first = engine.parse("text", "Hello world");
	first.share(1);
	using second = engine.fromYjs(first.encodeState(), 2);

	first.select({ path: [0], offset: 0 }, { path: [0], offset: 5 });
	first.commands.setColor({ color: "#e8590c" });
	second.select({ path: [0], offset: 3 }, { path: [0], offset: 11 });
	second.commands.toggleHighlight({ color: "#fef08a" });

	const fromFirst = first.takeUpdates();
	const fromSecond = second.takeUpdates();
	fromFirst.forEach((update) => second.applyUpdate(update));
	fromSecond.forEach((update) => first.applyUpdate(update));

	assert.deepEqual(first.toJson(), second.toJson());
	assert.equal(
		first.export("html"),
		'<p><span style="color: #e8590c">Hel</span><span style="color: #e8590c"><mark data-color="#fef08a" style="background-color: #fef08a; color: inherit">lo</mark></span><mark data-color="#fef08a" style="background-color: #fef08a; color: inherit"> world</mark></p>',
	);
});
