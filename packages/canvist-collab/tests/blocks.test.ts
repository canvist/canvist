/**
 * Details, callouts, and math through `@canvist/collab/full`: peers editing
 * them concurrently converge, through a JavaScript Yjs peer.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import * as Y from "yjs";

import {
	callout,
	createEngine,
	details,
	mathematics,
	starterKit,
} from "../src/full.ts";

const blocks = [...starterKit, details, callout, mathematics] as const;

test("peers editing blocks concurrently converge, through a JavaScript Yjs peer", async () => {
	using engine = await createEngine(blocks);
	using first = engine.parse(
		"markdown",
		"> [!NOTE]\n> Careful\n\n<details open>\n<summary>Sum</summary>\n\nBody\n\n</details>",
	);
	first.share(1);

	// The second peer joins through a plain JavaScript Yjs document, which
	// carries the block types and attributes it does not understand.
	const relay = new Y.Doc();
	Y.applyUpdate(relay, first.encodeState());
	const roots = relay.getArray<Y.Map<unknown>>("blocks");
	assert.equal(roots.get(0)?.get("type"), "callout");
	assert.equal(
		(roots.get(0)?.get("attrs") as Y.Map<unknown>).get("type"),
		"info",
	);
	assert.equal(
		(roots.get(1)?.get("attrs") as Y.Map<unknown>).get("open"),
		true,
	);
	using second = engine.fromYjs(Y.encodeStateAsUpdate(relay), 2);
	assert.deepEqual(second.toJson(), first.toJson());

	first.select({ path: [0, 0], offset: 0 });
	first.commands.setCalloutType({ type: "warning" });
	first.select({ path: [1, 0], offset: 0 });
	first.commands.toggleDetailsOpen();
	second.select({ path: [0, 0], offset: 7 });
	second.insertText(" now");
	second.select({ path: [1, 1, 0], offset: 4 });
	second.commands.insertInlineMath({ latex: "x" });

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
		"> [!WARNING]\n> Careful now\n\n<details>\n<summary>Sum</summary>\n\nBody$x$\n\n</details>",
	);
	using third = engine.fromYjs(Y.encodeStateAsUpdate(relay), 3);
	assert.deepEqual(third.toJson(), first.toJson());
	assert.deepEqual(first.validate(), { rejectedBlocks: 0, waiting: false });
});
