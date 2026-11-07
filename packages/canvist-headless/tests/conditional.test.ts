/**
 * Conditional text through `@canvist/headless/full`: the descriptor matches
 * Rust, selected text and the caret take conditions, and HTML keeps them.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { conditional, createEngine, isCanvistError } from "../src/full.ts";
import { comparable } from "./descriptors.ts";

test("the conditional descriptor matches what Rust reports", async () => {
	using engine = await createEngine([conditional]);
	const reported = comparable(engine.metadata()).filter((entry) =>
		entry.name === "conditional"
	);

	assert.deepEqual(reported, comparable([conditional]));
});

test("one post, a handle per network", async () => {
	using engine = await createEngine([conditional]);
	const doc = engine.parse("text", "Thanks ");

	doc.select({ path: [0], offset: 7 });
	assert.equal(
		doc.commands.insertConditional({
			when: "bluesky",
			text: "@ada.bsky.social",
		}),
		true,
	);
	assert.equal(
		doc.commands.insertConditional({ when: "instagram", text: "@ada_builds" }),
		true,
	);

	assert.equal(
		doc.export("html"),
		'<p>Thanks <span data-type="conditional" data-when="bluesky">@ada.bsky.social</span><span data-type="conditional" data-when="instagram">@ada_builds</span></p>',
	);
	assert.equal(
		engine.parse("html", doc.export("html")).export("html"),
		doc.export("html"),
	);
});

test("selected text is made conditional, and read back", async () => {
	using engine = await createEngine([conditional]);
	const doc = engine.parse("text", "Hello everyone");

	doc.select({ path: [0], offset: 6 }, { path: [0], offset: 14 });
	assert.equal(doc.query.isConditional(), false);
	assert.equal(doc.commands.setConditional({ when: "linkedin" }), true);
	assert.equal(doc.query.conditionalWhen(), "linkedin");
	assert.deepEqual(doc.toJson().content?.[0]?.content?.[1], {
		type: "text",
		text: "everyone",
		marks: [{ type: "conditional", attrs: { when: "linkedin" } }],
	});

	assert.equal(doc.commands.unsetConditional(), true);
	assert.equal(doc.query.conditionalWhen(), null);
	assert.equal(doc.export("markdown"), "Hello everyone");
});

test("a condition must say something", async () => {
	using engine = await createEngine([conditional]);
	const doc = engine.parse("text", "Hi");

	doc.select({ path: [0], offset: 0 }, { path: [0], offset: 2 });
	assert.throws(
		() => doc.commands.setConditional({ when: " " }),
		(error) => isCanvistError(error) && error.code === "invalidArgs",
	);
});
