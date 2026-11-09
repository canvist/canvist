/**
 * The typed classes over a fake module that records every call, so they are
 * tested without one: each method reaches the module with the right
 * arguments and hands back what it answers.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { bold, heading } from "../src/extensions.generated.ts";
import {
	bundled,
	Doc,
	Engine,
	type Extensions,
	isCanvistError,
	type WasmDoc,
	type WasmEngine,
} from "../src/mod.ts";

/** A document that answers each call with `answers[name]`, recording the call. */
class FakeDoc implements WasmDoc {
	readonly calls: unknown[][] = [];
	readonly answers: Record<string, unknown>;
	freed = false;

	constructor(answers: Record<string, unknown> = {}) {
		this.answers = answers;
	}

	#call<T>(name: string, ...args: unknown[]): T {
		this.calls.push([name, ...args]);
		return this.answers[name] as T;
	}

	run(name: string, args: string): boolean {
		return this.#call("run", name, args);
	}
	can(name: string, args: string): boolean {
		return this.#call("can", name, args);
	}
	query(name: string, args: string): string {
		return this.#call("query", name, args);
	}
	export(format: string): string {
		return this.#call("export", format);
	}
	selection(): string {
		return this.#call("selection");
	}
	select(
		anchorPath: Uint32Array,
		anchorOffset: number,
		headPath: Uint32Array,
		headOffset: number,
	): void {
		this.#call(
			"select",
			[...anchorPath],
			anchorOffset,
			[...headPath],
			headOffset,
		);
	}
	selectAll(): void {
		this.#call("selectAll");
	}
	insertText(text: string): boolean {
		return this.#call("insertText", text);
	}
	typeText(text: string): boolean {
		return this.#call("typeText", text);
	}
	deleteSelection(): boolean {
		return this.#call("deleteSelection");
	}
	press(key: string): boolean {
		return this.#call("press", key);
	}
	paste(format: string, content: string): boolean {
		return this.#call("paste", format, content);
	}
	undo(): boolean {
		return this.#call("undo");
	}
	redo(): boolean {
		return this.#call("redo");
	}
	snapshot(): string {
		return this.#call("snapshot");
	}
	free(): void {
		this.freed = true;
	}
}

/** An engine whose documents are {@link FakeDoc}s answering `answers`. */
class FakeEngine implements WasmEngine {
	readonly docs: FakeDoc[] = [];
	readonly calls: unknown[][] = [];
	readonly answers: Record<string, unknown>;
	freed = false;

	constructor(answers: Record<string, unknown> = {}) {
		this.answers = answers;
	}

	#doc(name: string, ...args: unknown[]): FakeDoc {
		this.calls.push([name, ...args]);
		const doc = new FakeDoc(this.answers);
		this.docs.push(doc);
		return doc;
	}

	metadata(): string {
		return JSON.stringify([{ name: "bold" }]);
	}
	create(): FakeDoc {
		return this.#doc("create");
	}
	parse(format: string, input: string): FakeDoc {
		return this.#doc("parse", format, input);
	}
	restore(snapshot: string): FakeDoc {
		return this.#doc("restore", snapshot);
	}
	free(): void {
		this.freed = true;
	}
}

const extensions = [bold, heading] as const;

test("an engine lists its extensions and hands out documents", () => {
	const wasm = new FakeEngine();
	const engine = new Engine(wasm, extensions);

	assert.deepEqual(
		engine.extensions.map(({ name }) => name),
		["undoInputRule", "paragraph", "doc", "baseKeymap", "bold", "heading"],
	);
	assert.deepEqual(engine.metadata(), [{ name: "bold" }]);
	assert.ok(engine.create() instanceof Doc);
	engine.parse("markdown", "# Hi");
	engine.fromJson({ type: "doc", content: [] } as never);
	engine.restore("{}");
	assert.deepEqual(wasm.calls, [
		["create"],
		["parse", "markdown", "# Hi"],
		["parse", "json", '{"type":"doc","content":[]}'],
		["restore", "{}"],
	]);

	engine[Symbol.dispose]();
	assert.equal(wasm.freed, true);
});

test("an engine wraps documents in the class it is given", () => {
	class Wrapped<E extends Extensions> extends Doc<E> {}
	type Kit = typeof extensions;
	const wrapped: unknown[] = [];
	const engine = new Engine<Kit, Wrapped<Kit>>(
		new FakeEngine(),
		extensions,
		(wasm, registered) => {
			wrapped.push(registered.length);
			return new Wrapped(wasm, registered);
		},
	);

	assert.ok(engine.create() instanceof Wrapped);
	assert.ok(engine.parse("text", "x") instanceof Wrapped);
	assert.ok(engine.restore("{}") instanceof Wrapped);
	assert.deepEqual(wrapped, [6, 6, 6]);
});

test("typed commands, checks, and queries send JSON arguments", () => {
	const wasm = new FakeEngine({ run: true, can: false, query: "2" });
	const doc = new Engine(wasm, extensions).create();
	const fake = wasm.docs[0] as FakeDoc;

	assert.equal(doc.commands.toggleBold(), true);
	assert.equal(doc.commands.setHeading({ level: 2 }), true);
	assert.equal(doc.can.toggleBold(), false);
	assert.equal(doc.query.isBold(), 2);
	assert.equal(doc.run("toggleBold"), true);
	assert.equal(doc.canRun("setHeading", { level: 3 }), false);
	assert.equal(doc.ask("isBold"), 2);
	assert.deepEqual(fake.calls, [
		["run", "toggleBold", "null"],
		["run", "setHeading", '{"level":2}'],
		["can", "toggleBold", "null"],
		["query", "isBold", "null"],
		["run", "toggleBold", "null"],
		["can", "setHeading", '{"level":3}'],
		["query", "isBold", "null"],
	]);
});

test("editing, export, and the selection reach the module", () => {
	const wasm = new FakeEngine({
		export: '{"type":"doc"}',
		selection: '{"anchor":{"path":[0],"offset":1}}',
		insertText: true,
		typeText: true,
		deleteSelection: true,
		press: true,
		paste: true,
		undo: true,
		redo: false,
		snapshot: "{}",
	});
	const doc = new Engine(wasm, extensions).create();
	const fake = wasm.docs[0] as FakeDoc;

	assert.equal(doc.export("json"), '{"type":"doc"}');
	assert.deepEqual(doc.toJson(), { type: "doc" });
	assert.deepEqual(doc.selection, { anchor: { path: [0], offset: 1 } });
	doc.select({ path: [0], offset: 1 });
	doc.select({ path: [0], offset: 1 }, { path: [1], offset: 2 });
	doc.selectAll();
	assert.equal(doc.insertText("a"), true);
	assert.equal(doc.typeText("b"), true);
	assert.equal(doc.deleteSelection(), true);
	assert.equal(doc.press("Enter"), true);
	assert.equal(doc.paste("html", "<p>c</p>"), true);
	assert.equal(doc.undo(), true);
	assert.equal(doc.redo(), false);
	assert.equal(doc.snapshot(), "{}");
	assert.deepEqual(fake.calls.slice(3), [
		["select", [0], 1, [0], 1],
		["select", [0], 1, [1], 2],
		["selectAll"],
		["insertText", "a"],
		["typeText", "b"],
		["deleteSelection"],
		["press", "Enter"],
		["paste", "html", "<p>c</p>"],
		["undo"],
		["redo"],
		["snapshot"],
	]);

	doc[Symbol.dispose]();
	assert.equal(fake.freed, true);
});

test("canvist errors are told apart from other errors", () => {
	const error = Object.assign(new Error("no"), {
		name: "CanvistError",
		code: "unknownName",
	});

	assert.equal(isCanvistError(error), true);
	assert.equal(isCanvistError(new Error("no")), false);
	assert.equal(isCanvistError({ name: "CanvistError", code: "x" }), false);
	assert.equal(
		isCanvistError(Object.assign(new Error("no"), { name: "CanvistError" })),
		false,
	);
});

test("a bundled module is read from disk, or fetched from elsewhere", async () => {
	const here = new URL(import.meta.url);
	const bytes = await bundled(here);
	assert.ok(bytes instanceof Uint8Array);
	assert.match(new TextDecoder().decode(bytes), /a bundled module is read/);

	const remote = new URL("https://example.com/canvist.wasm");
	assert.equal(await bundled(remote), remote);
});
