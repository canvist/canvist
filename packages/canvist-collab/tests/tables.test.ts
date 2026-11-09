/**
 * Tables through `@canvist/collab/full`: peers changing a table while others
 * type converge, through a JavaScript Yjs peer.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import * as Y from "yjs";

import { createEngine, starterKit, table } from "../src/full.ts";

const withTable = [...starterKit, table] as const;

test("peers changing a table while others type converge, through a JavaScript Yjs peer", async () => {
	using engine = await createEngine(withTable);
	using first = engine.parse("markdown", "| a | b |\n| --- | --- |\n| c | d |");
	first.share(1);

	const relay = new Y.Doc();
	Y.applyUpdate(relay, first.encodeState());
	const blocks = relay.getArray<Y.Map<unknown>>("blocks");
	assert.equal(blocks.get(0)?.get("type"), "table");
	const rows = blocks.get(0)?.get("children") as Y.Array<Y.Map<unknown>>;
	assert.equal(rows.length, 2);
	assert.equal(
		(rows.get(0).get("children") as Y.Array<Y.Map<unknown>>).get(0).get("type"),
		"tableHeader",
	);
	using second = engine.fromYjs(Y.encodeStateAsUpdate(relay), 2);

	first.select({ path: [0, 0, 0, 0], offset: 0 });
	first.commands.addRowAfter();
	second.select({ path: [0, 1, 1, 0], offset: 1 });
	second.insertText("!");
	second.commands.setCellAttribute({ name: "align", value: "right" });

	const fromFirst = first.takeUpdates();
	const fromSecond = second.takeUpdates();
	fromSecond.forEach((update) => first.applyUpdate(update));
	fromFirst.forEach((update) => second.applyUpdate(update));
	fromFirst.concat(fromSecond).forEach((update) =>
		Y.applyUpdate(relay, update)
	);

	assert.deepEqual(first.toJson(), second.toJson());
	assert.equal(
		first.export("markdown"),
		"| a | b |\n| --- | --- |\n|  |  |\n| c | d! |",
	);
	using third = engine.fromYjs(Y.encodeStateAsUpdate(relay), 3);
	assert.deepEqual(third.toJson(), first.toJson());
	assert.deepEqual(first.validate(), { rejectedBlocks: 0, waiting: false });
});
