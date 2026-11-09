/**
 * The TypeScript API beyond the conformance fixtures: extension subsets,
 * errors, and disposal.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import {
	allExtensions,
	createEngine as createFullEngine,
	emoji,
	highlight,
} from "../src/full.ts";
import {
	bold,
	createEngine,
	heading,
	isCanvistError,
	starterKit,
} from "../src/mod.ts";
import { comparable } from "./descriptors.ts";

test("an engine offers exactly its extensions' commands and queries", async () => {
	using engine = await createEngine([bold, heading]);
	using doc = engine.create();

	assert.deepEqual(Object.keys(doc.commands).sort(), [
		"setBold",
		"setHeading",
		"setParagraph",
		"toggleBold",
		"toggleHeading",
		"unsetBold",
	]);
	assert.deepEqual(Object.keys(doc.query).sort(), [
		"headingLevel",
		"isBold",
		"isParagraph",
	]);

	const names = (engine.metadata() as { name: string }[]).map((extension) =>
		extension.name
	).sort();
	assert.deepEqual(names, [
		"baseKeymap",
		"bold",
		"doc",
		"heading",
		"paragraph",
		"undoInputRule",
	]);
});

test("the generated descriptors match what Rust reports", async () => {
	using engine = await createEngine();
	const metadata = engine.metadata();

	// The descriptions are in the metadata, and only there.
	assert.ok(
		metadata.some((extension) => (extension.commands[0]?.doc ?? "") !== ""),
	);
	assert.deepEqual(comparable(metadata), comparable(engine.extensions));
	assert.equal(engine.extensions.length, starterKit.length + 4);
});

test("the StarterKit module has only the StarterKit", async () => {
	await assert.rejects(
		createEngine([bold, highlight] as unknown as [typeof bold]),
		{ name: "CanvistError", code: "config" },
	);

	using full = await createFullEngine(allExtensions);
	assert.equal(full.extensions.length, allExtensions.length + 4);
	assert.deepEqual(
		(full.metadata() as { name: string }[]).map((extension) => extension.name)
			.sort(),
		full.extensions.map((extension) => extension.name).sort(),
	);
});

test("the full module loads the emoji table with the first emoji engine", async () => {
	using engine = await createFullEngine([emoji]);
	using doc = engine.create();

	doc.typeText(":smile: ");
	assert.equal(doc.export("text"), "😄 ");
	assert.deepEqual(
		doc.query.searchEmoji({ query: "thumbsu" }).map((item) => item.name),
		["thumbsup"],
	);
});

test("failures are CanvistErrors with stable codes", async () => {
	const engine = await createEngine();

	const failure = (() => {
		try {
			engine.parse("json", "{ not json");
		} catch (error) {
			return error;
		}
	})();

	assert.ok(isCanvistError(failure));
	assert.equal(failure.code, "parse");
	assert.throws(() => engine.create().commands.setHeading({ level: 9 as 1 }), {
		name: "CanvistError",
		code: "invalidArgs",
	});
	assert.equal(isCanvistError(new Error("plain")), false);
});

test("documents survive snapshots and selections use UTF-16 offsets", async () => {
	const engine = await createEngine();
	const doc = engine.parse("text", "a😀b");
	doc.select({ path: [0], offset: 3 }, { path: [0], offset: 4 });
	doc.insertText("c");

	const restored = engine.restore(doc.snapshot());
	assert.equal(restored.export("text"), "a😀c");
	assert.deepEqual(restored.selection, {
		anchor: { path: [0], offset: 4 },
		head: { path: [0], offset: 4 },
	});
});
