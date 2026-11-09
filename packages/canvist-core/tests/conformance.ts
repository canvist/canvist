/**
 * Runs the language-neutral fixtures in `conformance/fixtures` against a
 * TypeScript engine module, under `node --test` and `deno test` alike. See
 * `conformance/readme.md` for the format. `@canvist/headless` runs it
 * without the `collab` feature and `@canvist/collab` with it, so every
 * module canvist ships passes the same cases.
 *
 * @module
 */
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";

import {
	type CommandName,
	type Doc,
	type Engine,
	type Extensions,
	type Format,
	isCanvistError,
	type Position,
	type QueryName,
	type Registered,
} from "@canvist/core";
import { starterKit } from "@canvist/core/extensions";
import { allExtensions } from "@canvist/core/extras";

type Json = null | boolean | number | string | readonly Json[] | {
	readonly [key: string]: Json;
};
type Fields = { readonly [key: string]: Json };
/**
 * Cases are typed as if every extension were registered: a fixture only
 * names commands its case registers, and Rust rejects any other name.
 */
type Every = typeof allExtensions;
type Kit = Registered<Every>;

/** The Yjs calls `collab` cases make, which only collaborating modules have. */
interface Shared {
	share(clientId: number): void;
	applyUpdate(update: Uint8Array): boolean;
	encodeState(): Uint8Array;
	takeUpdates(): Uint8Array[];
	validate(): { readonly rejectedBlocks: number; readonly waiting: boolean };
	receiveSync(message: Uint8Array): {
		readonly reply: readonly Uint8Array[];
		readonly broadcast: readonly Uint8Array[];
		readonly changed: boolean;
	};
}
type KitDoc = Doc<Every> & Shared;
type KitEngine = Engine<Every, KitDoc> & {
	fromYjs(state: Uint8Array, clientId: number): KitDoc;
};

/** Creates an engine with the given extensions. */
type CreateEngine = (extensions: Extensions) => Promise<unknown>;

/** The module under test. */
export interface Module {
	/** Creates engines from the StarterKit module. */
	readonly createEngine: CreateEngine;
	/** Creates engines from the module with every extension. */
	readonly createFullEngine: CreateEngine;
	/** The optional features the module has: `collab`, `extensions`. */
	readonly features: readonly string[];
}

const FIXTURES = new URL("../../../conformance/fixtures/", import.meta.url);

/**
 * Register a test for every fixture case, skipping the cases that need a
 * feature `module` lacks. Engines are created before any test runs, so
 * registering tests never awaits.
 */
export async function conformance(module: Module): Promise<void> {
	const files = readdirSync(FIXTURES).filter((name) => name.endsWith(".json"))
		.sort();
	const fixtures = files.map((file) => {
		const fixture = JSON.parse(
			readFileSync(new URL(file, FIXTURES), "utf8"),
		) as { cases: Fields[] };

		return { file, cases: fixture.cases };
	});
	const engines = new Map<string, KitEngine>();

	for (const { cases } of fixtures) {
		for (const testCase of cases) {
			const names = (testCase.extensions ?? []) as string[];

			if (!engines.has(names.join(","))) {
				engines.set(names.join(","), await caseEngine(module, names));
			}
		}
	}

	for (const { file, cases } of fixtures) {
		for (const testCase of cases) {
			const requires = (testCase.requires ?? []) as string[];
			const skip = !requires.every((feature) =>
				module.features.includes(feature)
			);
			const names = (testCase.extensions ?? []) as string[];
			const engine = engines.get(names.join(",")) as KitEngine;
			test(
				`${file}: ${String(testCase.name)}`,
				{ skip },
				() => runCase(engine, testCase),
			);
		}
	}
}

/**
 * The StarterKit plus the `extensions` a case lists, by name: from the
 * StarterKit module when the case lists none, so both modules run the
 * fixtures, and from the full module otherwise.
 */
async function caseEngine(
	module: Module,
	names: readonly string[],
): Promise<KitEngine> {
	const extra = allExtensions.filter((extension) =>
		names.includes(extension.name)
	);
	const engine = extra.length === 0
		? await module.createEngine(starterKit)
		: await module.createFullEngine([...starterKit, ...extra]);

	return engine as unknown as KitEngine;
}

function runCase(engine: KitEngine, testCase: Fields): void {
	const input = (testCase.input ?? {}) as Fields;

	if (typeof input.error === "string") {
		expectError(() => load(engine, input), input.error);

		return;
	}

	let doc = load(engine, input);

	for (const step of (testCase.steps ?? []) as Fields[]) {
		doc = runStep(engine, doc, step);
	}

	check(doc, (testCase.expect ?? {}) as Fields);
}

function load(engine: KitEngine, input: Fields): KitDoc {
	switch (input.format) {
		case undefined:
			return engine.create();
		case "json":
			return engine.parse("json", JSON.stringify(input.content));
		case "yjs":
			return engine.fromYjs(base64(input.content), Number(input.clientId));
		default:
			return engine.parse(input.format as Format, String(input.content));
	}
}

/** Run a step, returning the document later steps use. */
function runStep(engine: KitEngine, doc: KitDoc, step: Fields): KitDoc {
	if (typeof step.error === "string") {
		const before = doc.export("json");
		expectError(() => apply(doc, step), step.error);
		assert.equal(doc.export("json"), before, "a failed step changes nothing");

		return doc;
	}

	switch (step.op) {
		case "expect":
			check(doc, step);

			return doc;
		case "snapshotRoundTrip":
			return engine.restore(JSON.stringify(JSON.parse(doc.snapshot())));
		case "rejoin":
			return engine.fromYjs(doc.encodeState(), Number(step.clientId));
	}

	const outcome = apply(doc, step);
	const expected = expectedOutcome(step);

	if (expected !== undefined) {
		assert.deepEqual(outcome, expected, `${String(step.op)} outcome`);
	}

	return doc;
}

/** Where a step declares the outcome it expects. */
const OUTCOMES: Record<string, (step: Fields) => Json | undefined> = {
	receiveSync: (
		step,
	) => [step.reply ?? null, step.broadcast ?? null, step.changed ?? null],
	validate: (step) => [step.rejectedBlocks ?? null, step.waiting ?? null],
	takeUpdates: (step) => step.count,
	can: (step) => step.expect,
	query: (step) => step.expect,
	applyUpdate: (step) => step.changed,
};

function expectedOutcome(step: Fields): Json | undefined {
	const outcome = OUTCOMES[String(step.op)];

	return outcome === undefined ? step.applied : outcome(step);
}

type Operation = (doc: KitDoc, step: Fields) => Json;

const command = (step: Fields) => String(step.command) as CommandName<Kit>;
const args = (step: Fields) => step.args as Record<string, unknown> | undefined;
const text = (step: Fields) => String(step.text ?? "");

/** Every operation, each returning what it reports. */
const OPERATIONS: Record<string, Operation> = {
	select: (doc, step) => {
		const anchor = position(step.anchor);
		doc.select(anchor, step.head === undefined ? anchor : position(step.head));

		return true;
	},
	selectAll: (doc) => {
		doc.selectAll();

		return true;
	},
	insertText: (doc, step) => doc.insertText(text(step)),
	typeText: (doc, step) => doc.typeText(text(step)),
	deleteSelection: (doc) => doc.deleteSelection(),
	press: (doc, step) => doc.press(String(step.key)),
	paste: (doc, step) => doc.paste(step.format as Format, String(step.content)),
	run: (doc, step) => doc.run(command(step), args(step) as never),
	can: (doc, step) => doc.canRun(command(step), args(step) as never),
	query: (doc, step) =>
		doc.ask(String(step.query) as QueryName<Kit>, args(step) as never) as Json,
	undo: (doc) => doc.undo(),
	redo: (doc) => doc.redo(),
	share: (doc, step) => {
		doc.share(Number(step.clientId));

		return true;
	},
	applyUpdate: (doc, step) => doc.applyUpdate(base64(step.update)),
	takeUpdates: (doc) => doc.takeUpdates().length,
	validate: (doc) => {
		const validation = doc.validate();

		return [validation.rejectedBlocks, validation.waiting];
	},
	receiveSync: (doc, step) => {
		const sync = doc.receiveSync(base64(step.message));

		return [sync.reply.length, sync.broadcast.length, sync.changed];
	},
};

function apply(doc: KitDoc, step: Fields): Json {
	const operation = OPERATIONS[String(step.op)];
	assert.ok(operation, `unknown op ${String(step.op)}`);

	return operation(doc, step);
}

function expectError(run: () => unknown, code: Json): void {
	assert.throws(run, (error: unknown) => {
		assert.ok(
			isCanvistError(error),
			`expected a CanvistError, got ${String(error)}`,
		);
		assert.equal(error.code, code);
		return true;
	});
}

function check(doc: KitDoc, expect: Fields): void {
	for (const format of ["html", "markdown", "text"] as const) {
		if (format in expect) {
			assert.equal(doc.export(format), expect[format], format);
		}
	}

	if ("json" in expect) {
		assert.deepEqual(
			stripIds(JSON.parse(doc.export("json")) as Json),
			stripIds(expect.json ?? null),
			"json",
		);
	}

	if ("selection" in expect) {
		assert.deepEqual(doc.selection, expect.selection, "selection");
	}
}

/** Remove `id` from every `attrs` object, then every `attrs` left empty. */
function stripIds(json: Json): Json {
	if (Array.isArray(json)) {
		return json.map(stripIds);
	}

	if (json === null || typeof json !== "object") {
		return json;
	}

	const fields: Record<string, Json> = {};

	for (const [key, value] of Object.entries(json as Fields)) {
		if (key !== "attrs") {
			fields[key] = stripIds(value);
			continue;
		}

		const { id: _id, ...attrs } = value as Fields;

		if (Object.keys(attrs).length > 0) {
			fields.attrs = attrs;
		}
	}

	return fields;
}

function position(json: Json | undefined): Position {
	const fields = json as Fields;
	return { path: fields.path as number[], offset: fields.offset as number };
}

function base64(json: Json | undefined): Uint8Array {
	return Uint8Array.from(
		atob(String(json)),
		(character) => character.charCodeAt(0),
	);
}
