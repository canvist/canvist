/**
 * Mentions, hashtags, and emoji through the TypeScript API: typed commands
 * and query results, engines without them, and error codes. Editing them
 * together as Yjs peers is in `@canvist/collab`'s tests.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import {
	bold,
	createEngine,
	emoji,
	hashtag,
	isCanvistError,
	mention,
} from "../src/full.ts";

const atoms = [mention, hashtag, emoji] as const;

/** The code of the CanvistError `run` throws. */
function errorCode(run: () => unknown): string {
	try {
		run();
	} catch (error) {
		assert.ok(isCanvistError(error), `not a CanvistError: ${error}`);
		return error.code;
	}

	assert.fail("expected a CanvistError");
}

test("typed commands insert atoms and typed queries describe them", async () => {
	using engine = await createEngine(atoms);
	using doc = engine.create();

	doc.typeText("Hi @ad");
	const suggestion: { readonly char: string; readonly query: string } | null =
		doc.query.mentionSuggestion();
	assert.deepEqual(suggestion, { char: "@", query: "ad" });
	assert.equal(doc.can.insertMention({ id: "7" }), true);
	assert.equal(doc.commands.insertMention({ id: "7", label: "Ada" }), true);
	assert.equal(doc.query.mentionSuggestion(), null);

	doc.typeText(":tad");
	const found: readonly { readonly name: string; readonly emoji: string }[] =
		doc.query.searchEmoji({ query: "tad", limit: 1 });
	assert.deepEqual(found, [{ name: "tada", emoji: "🎉" }]);
	assert.equal(doc.commands.setEmoji({ name: found[0]?.name ?? "" }), true);
	assert.equal(doc.commands.setEmoji({ name: "not_an_emoji" }), false);

	doc.typeText("#ca");
	assert.deepEqual(doc.query.hashtagSuggestion(), { char: "#", query: "ca" });
	assert.equal(doc.commands.dismissHashtagSuggestion(), true);
	assert.equal(doc.query.hashtagSuggestion(), null);
	assert.equal(doc.commands.dismissHashtagSuggestion(), false);
	doc.typeText(" ");
	doc.commands.insertHashtag({ tag: "#canvas" });
	const tags: readonly string[] = doc.query.hashtags();
	assert.deepEqual(
		tags,
		["ca", "canvas"],
		"a dismissed suggestion is still a tag once its word ends",
	);

	assert.equal(doc.export("text"), "Hi @Ada 🎉 #ca #canvas ");
	assert.deepEqual(doc.selection, {
		anchor: { path: [0], offset: 19 },
		head: { path: [0], offset: 19 },
	}, "an atom is one UTF-16 unit");
});

test("the default search limit is ten and Escape dismisses through press", async () => {
	using engine = await createEngine([emoji]);
	using doc = engine.create();

	assert.equal(doc.query.searchEmoji({ query: "face" }).length, 10);
	doc.typeText(":sm");
	assert.equal(doc.press("Escape"), true);
	assert.equal(doc.query.emojiSuggestion(), null);
	doc.typeText("ile:");
	assert.equal(doc.export("text"), "😄", "the shortcode still converts");
	assert.equal(doc.press("Backspace"), true);
	assert.equal(
		doc.export("text"),
		":smile:",
		"Backspace undoes the conversion",
	);
	assert.equal(doc.undo(), true);
	assert.equal(doc.export("text"), "😄");
});

test("an engine offers only the atoms it registered", async () => {
	using engine = await createEngine([bold, mention]);
	using doc = engine.create();

	assert.deepEqual(
		Object.keys(doc.commands).filter((name) => !name.endsWith("Bold")).sort(),
		["dismissMentionSuggestion", "insertMention", "setParagraph"],
	);
	assert.deepEqual(
		Object.keys(doc.query).filter((name) => name !== "isBold").sort(),
		["isParagraph", "mentionSuggestion"],
	);
	// Names from outside the type system, as a host picking them at runtime.
	const untyped = doc as unknown as {
		run(name: string, args?: unknown): boolean;
	};
	assert.equal(
		errorCode(() => untyped.run("setEmoji", { name: "smile" })),
		"unknownName",
	);
	doc.typeText(":smile: #tag ");
	assert.equal(doc.export("html"), "<p>:smile: #tag </p>");

	const metadata = engine.metadata();
	const node = metadata.find((extension) => extension.name === "mention")
		?.nodes[0];
	assert.equal(node?.name, "mention");
	assert.deepEqual(node?.attrs.map((attr) => attr.name), [
		"id",
		"label",
		"mentionSuggestionChar",
	]);
});

test("invalid arguments fail with stable codes and change nothing", async () => {
	using engine = await createEngine(atoms);
	using doc = engine.create();
	doc.typeText("@a");
	const before = doc.export("json");

	assert.equal(
		errorCode(() => doc.commands.insertMention({} as never)),
		"invalidArgs",
	);
	assert.equal(
		errorCode(() => doc.commands.insertMention({ id: 7 } as never)),
		"invalidArgs",
	);
	assert.equal(
		errorCode(() => doc.commands.insertHashtag({ tag: ["a"] } as never)),
		"invalidArgs",
	);
	assert.equal(
		errorCode(() => doc.commands.setEmoji({} as never)),
		"invalidArgs",
	);
	assert.equal(
		errorCode(() =>
			doc.query.searchEmoji({ query: "a", limit: "ten" } as never)
		),
		"invalidArgs",
	);
	assert.equal(
		errorCode(() => doc.query.searchEmoji({ query: "a", limit: 1.5 })),
		"invalidArgs",
	);
	assert.equal(doc.export("json"), before);
	const unknown = { name: "emojis", commands: [], queries: [] };
	await assert.rejects(
		createEngine([unknown] as never),
		(error) => isCanvistError(error) && error.code === "config",
	);
});

test("TipTap JSON atoms load through fromJson and export as HTML", async () => {
	using engine = await createEngine(atoms);
	using doc = engine.fromJson({
		type: "doc",
		content: [{
			type: "paragraph",
			content: [
				{ type: "mention", attrs: { id: "u1" } },
				{ type: "text", text: " " },
				{ type: "emoji", attrs: { name: "smile" } },
				{
					type: "text",
					text: " #x",
					marks: [{ type: "hashtag", attrs: { tag: "x" } }],
				},
			],
		}],
	});

	assert.equal(doc.export("text"), "@u1 :smile: #x");
	assert.equal(
		doc.export("html"),
		'<p><span data-type="mention" data-id="u1">@u1</span> <span data-type="emoji" data-name="smile">:smile:</span><span data-type="hashtag" data-tag="x"> #x</span></p>',
	);
	assert.deepEqual(doc.query.hashtags(), ["x"]);
});
