/**
 * The editing helpers beyond the StarterKit through the TypeScript API:
 * their generated descriptors, typed commands and queries, and stable
 * errors.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import {
	allExtensions,
	characterCount,
	createEngine,
	heading,
	isCanvistError,
	placeholder,
	starterKit,
	textAlign,
	typography,
} from "../src/full.ts";
import { comparable } from "./descriptors.ts";

const editing = [textAlign, typography, characterCount, placeholder] as const;

/** The code a failing call throws, or `undefined` when it succeeds. */
function errorCode(call: () => unknown): string | undefined {
	try {
		call();
	} catch (error) {
		assert.ok(isCanvistError(error), `${String(error)} is a CanvistError`);
		return error.code;
	}

	return undefined;
}

test("every editing extension has a descriptor beyond the StarterKit", () => {
	const kit = new Set<string>(starterKit.map((extension) => extension.name));
	const all = new Set<string>(allExtensions.map((extension) => extension.name));

	for (const extension of editing) {
		assert.equal(kit.has(extension.name), false, extension.name);
		assert.equal(all.has(extension.name), true, extension.name);
	}

	assert.deepEqual(typography.commands, [], "typography only has input rules");
	assert.deepEqual(
		textAlign.nodes.map((node) => node.name).sort(),
		["heading", "paragraph"],
	);
});

test("the editing descriptors match what Rust reports", async () => {
	using engine = await createEngine([...starterKit, ...editing]);

	assert.deepEqual(
		comparable(engine.metadata()),
		comparable(engine.extensions),
	);
});

test("an engine without headings aligns only paragraphs", async () => {
	using engine = await createEngine([textAlign]);
	const metadata = engine.metadata();

	assert.deepEqual(
		metadata.find((extension) => extension.name === "textAlign")?.nodes.map((
			node,
		) => node.name),
		["paragraph"],
	);
});

test("typed commands and queries align, count, and show placeholders", async () => {
	using engine = await createEngine([...starterKit, ...editing]);
	using doc = engine.parse("markdown", "# Title\n\nBody");

	doc.selectAll();
	assert.equal(doc.can.setTextAlign({ alignment: "center" }), true);
	assert.equal(doc.commands.setTextAlign({ alignment: "center" }), true);
	assert.equal(doc.query.textAlign(), "center");
	assert.equal(doc.commands.toggleTextAlign({ alignment: "center" }), true);
	assert.equal(doc.query.textAlign(), null);
	doc.commands.setTextAlign({ alignment: "justify" });
	assert.equal(
		doc.export("html"),
		'<h1 style="text-align: justify">Title</h1><p style="text-align: justify">Body</p>',
	);
	assert.equal(doc.undo(), true);
	assert.equal(doc.query.textAlign(), null);

	assert.equal(doc.query.characters(), 9);
	assert.equal(doc.query.words(), 2);
	assert.equal(doc.query.characterLimit(), null);
	doc.commands.setCharacterLimit({ limit: 10 });
	doc.select({ path: [1], offset: 4 });
	doc.typeText("!!");
	assert.equal(doc.export("text"), "Title\nBody!", "the limit stops typing");

	doc.press("Enter");
	assert.equal(doc.query.placeholder(), "Write something …");
	doc.commands.setPlaceholder({ placeholder: "More?" });
	assert.equal(doc.query.placeholder(), "More?");
});

test("typography runs when typing, not when inserting or pasting", async () => {
	using engine = await createEngine([...starterKit, typography]);
	using doc = engine.create();

	doc.typeText('"Hi" -- 1/2 ');
	assert.equal(doc.export("text"), "“Hi” — ½ ");
	assert.equal(doc.press("Backspace"), true);
	assert.equal(doc.export("text"), "“Hi” — 1/2 ", "Backspace undoes the rule");
	doc.insertText("->");
	doc.paste("text", " (c)");
	assert.equal(doc.export("text"), "“Hi” — 1/2 -> (c)");
});

test("editing errors carry stable codes", async () => {
	using engine = await createEngine([...starterKit, ...editing]);
	using doc = engine.parse("text", "x");

	assert.equal(
		errorCode(() =>
			doc.commands.setTextAlign({ alignment: "middle" as "left" })
		),
		"invalidArgs",
	);
	assert.equal(
		errorCode(() => doc.commands.setCharacterLimit({ limit: -1 })),
		"invalidArgs",
	);
	assert.equal(
		errorCode(() => doc.commands.setCharacterLimit({ limit: 2 ** 32 })),
		"invalidArgs",
	);
	assert.equal(
		errorCode(() => doc.run("setPlaceholder", { placeholder: 1 } as never)),
		"invalidArgs",
	);
	assert.equal(doc.export("html"), "<p>x</p>");

	using withoutAlign = await createEngine([heading]);
	assert.equal(
		errorCode(() =>
			withoutAlign.create().run("setTextAlign" as "setHeading", { level: 1 })
		),
		"unknownName",
	);
});
