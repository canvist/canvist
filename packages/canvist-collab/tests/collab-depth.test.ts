/** Remote block nesting is bounded in the actual WebAssembly entry points. */
import assert from "node:assert/strict";
import { test } from "node:test";

import * as Y from "yjs";

import { createEngine, isCanvistError } from "../src/mod.ts";

/** Build shared maps iteratively so fixture construction uses bounded stack. */
function nest(
	array: Y.Array<Y.Map<unknown>>,
	depth: number,
	firstId = 0,
): Y.Array<Y.Map<unknown>> {
	let bottom = array;

	for (let id = firstId; id < firstId + depth; id++) {
		const map = new Y.Map<unknown>();
		bottom.push([map]);
		map.set("id", `1.${id.toString(36)}`);
		map.set("type", "blockquote");
		const children = new Y.Array<Y.Map<unknown>>();
		map.set("children", children);
		bottom = children;
	}

	return bottom;
}

/** Assert a handled collaboration error rather than a WebAssembly trap. */
function isDepthError(error: unknown): boolean {
	return isCanvistError(error) && error.code === "collab" &&
		/shared block nesting exceeds the limit of 64 levels/.test(error.message);
}

test("fromYjs accepts trees at the nesting boundary", async (context) => {
	using engine = await createEngine();

	for (const depth of [63, 64]) {
		const peer = new Y.Doc();
		context.after(() => peer.destroy());
		peer.transact(() => nest(peer.getArray("blocks"), depth));
		using doc = engine.fromYjs(Y.encodeStateAsUpdate(peer), 2);
		assert.deepEqual(doc.validate(), { rejectedBlocks: 0, waiting: false });
	}
});

test("fromYjs rejects over-depth trees with a collaboration error", async (context) => {
	using engine = await createEngine();

	for (const depth of [65, 1024]) {
		const peer = new Y.Doc();
		context.after(() => peer.destroy());
		peer.transact(() => nest(peer.getArray("blocks"), depth));
		assert.throws(
			() => engine.fromYjs(Y.encodeStateAsUpdate(peer), 2),
			isDepthError,
		);
	}
});

test("applyUpdate rejects incremental excess without changing the snapshot", async (context) => {
	using engine = await createEngine();
	const peer = new Y.Doc();
	context.after(() => peer.destroy());
	const bottom = nest(peer.getArray("blocks"), 64);
	using doc = engine.fromYjs(Y.encodeStateAsUpdate(peer), 2);
	Y.applyUpdate(peer, doc.encodeState());
	doc.takeUpdates();
	const before = doc.snapshot();
	const vector = Y.encodeStateVector(peer);
	nest(bottom, 1, 64);
	const update = Y.encodeStateAsUpdate(peer, vector);

	assert.throws(() => doc.applyUpdate(update), isDepthError);
	assert.deepEqual(doc.snapshot(), before);
	assert.deepEqual(doc.takeUpdates(), []);
	const rejectedVector = Y.encodeStateVector(peer);
	bottom.delete(0, 1);
	assert.equal(
		doc.applyUpdate(Y.encodeStateAsUpdate(peer, rejectedVector)),
		true,
	);
	assert.deepEqual(doc.validate(), { rejectedBlocks: 0, waiting: false });
});
