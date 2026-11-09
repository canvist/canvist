/**
 * The typed engine and document, over any canvist WebAssembly module: the
 * StarterKit one `@canvist/headless` loads, the one with every extension
 * `@canvist/headless/full` loads, or the collaborating ones
 * `@canvist/collab` loads. This module loads none of them itself.
 *
 * @module
 */
import type { WasmSource } from "./bundled.ts";
import { calls, encode } from "./calls.ts";
import { core } from "./extensions.generated.ts";
import type {
	CommandName,
	CommandParams,
	Commands,
	DocJson,
	ExtensionDescriptor,
	Extensions,
	Format,
	Position,
	Queries,
	QueryName,
	QueryParams,
	QueryResult,
	Range,
	Registered,
	StarterKit,
} from "./types.ts";
import type { WasmDoc, WasmEngine } from "./wasm.ts";

/** Options for `createEngine`. */
export interface EngineOptions {
	/** The client block ids are allocated under; collaborating peers need distinct ones. Default 1. */
	readonly client?: number;
	/** Where to load the WebAssembly module from, for runtimes without a file system. */
	readonly wasm?: WasmSource;
}

/**
 * Wraps a module's document in the class an engine hands out: {@link Doc},
 * or a subclass that adds what its module compiles in.
 *
 * @internal
 */
export type DocWrapper<E extends Extensions, D extends Doc<E>> = (
	wasm: WasmDoc,
	extensions: Registered<E>,
) => D;

/** A set of extensions that creates documents. Create one with `createEngine`. */
export class Engine<E extends Extensions, D extends Doc<E> = Doc<E>> {
	readonly #wasm: WasmEngine;
	readonly #extensions: Registered<E>;
	readonly #wrap: DocWrapper<E, D>;

	/** @internal Use `createEngine`, which loads the WebAssembly module first. */
	constructor(wasm: WasmEngine, extensions: E, wrap?: DocWrapper<E, D>) {
		this.#extensions = [...core, ...extensions];
		this.#wasm = wasm;
		this.#wrap = wrap ??
			((doc, registered) => new Doc<E>(doc, registered) as D);
	}

	/** The registered extensions, including the ones every engine has. */
	get extensions(): Registered<E> {
		return this.#extensions;
	}

	/**
	 * The registered extensions' metadata as Rust reports it: their
	 * descriptors, with every command, query, and field's `doc`.
	 */
	metadata(): ExtensionDescriptor[] {
		return JSON.parse(this.#wasm.metadata());
	}

	/** A document with one empty paragraph. */
	create(): D {
		return this.#wrap(this.#wasm.create(), this.#extensions);
	}

	/** Read `input` as `format`. */
	parse(format: Format, input: string): D {
		return this.#wrap(this.#wasm.parse(format, input), this.#extensions);
	}

	/** Read a ProseMirror or TipTap document that uses only the registered extensions. */
	fromJson(json: DocJson<Registered<E>>): D {
		return this.parse("json", JSON.stringify(json));
	}

	/** Recreate a document from {@link Doc.snapshot} text. */
	restore(snapshot: string): D {
		return this.#wrap(this.#wasm.restore(snapshot), this.#extensions);
	}

	/** Release the engine's memory now rather than when it is collected. */
	[Symbol.dispose](): void {
		this.#wasm.free();
	}
}

/** One document and the editor that changes it. Every call is one undo step. */
export class Doc<E extends Extensions> {
	readonly #wasm: WasmDoc;

	/** Run a registered command against the selection; returns whether it applied. */
	readonly commands: Commands<Registered<E>>;
	/** Whether a registered command would apply. Nothing changes. */
	readonly can: Commands<Registered<E>>;
	/** Answer a registered query. */
	readonly query: Queries<Registered<E>>;

	/** @internal Documents come from an {@link Engine}. */
	constructor(wasm: WasmDoc, extensions: Extensions) {
		this.#wasm = wasm;
		const commands = extensions.flatMap((extension) =>
			extension.commands.map((command) => command.name)
		);
		const queries = extensions.flatMap((extension) =>
			extension.queries.map((query) => query.name)
		);
		this.commands = calls(
			commands,
			(name, args) => this.#wasm.run(name, encode(args)),
		);
		this.can = calls(
			commands,
			(name, args) => this.#wasm.can(name, encode(args)),
		);
		this.query = calls(
			queries,
			(name, args) => JSON.parse(this.#wasm.query(name, encode(args))),
		);
	}

	/** Run the command named `name`, for callers that pick commands at runtime. */
	run<N extends CommandName<Registered<E>>>(
		name: N,
		...args: CommandParams<Registered<E>, N>
	): boolean {
		return this.#wasm.run(name, encode(args[0]));
	}

	/** Whether the command named `name` would apply. Nothing changes. */
	canRun<N extends CommandName<Registered<E>>>(
		name: N,
		...args: CommandParams<Registered<E>, N>
	): boolean {
		return this.#wasm.can(name, encode(args[0]));
	}

	/** Answer the query named `name`, for callers that pick queries at runtime. */
	ask<N extends QueryName<Registered<E>>>(
		name: N,
		...args: QueryParams<Registered<E>, N>
	): QueryResult<Registered<E>, N> {
		return JSON.parse(this.#wasm.query(name, encode(args[0])));
	}

	/** The document as `format`. */
	export(format: Format): string {
		return this.#wasm.export(format);
	}

	/** The document as ProseMirror JSON, with block ids in `attrs.id`. */
	toJson(): DocJson<Registered<E>> {
		return JSON.parse(this.#wasm.export("json"));
	}

	/** The text selection, or `null` for a block selection. */
	get selection(): Range | null {
		return JSON.parse(this.#wasm.selection());
	}

	/** Select from `anchor` to `head`, or place a caret at `anchor`. Offsets count UTF-16 code units. */
	select(anchor: Position, head: Position = anchor): void {
		this.#wasm.select(
			Uint32Array.from(anchor.path),
			anchor.offset,
			Uint32Array.from(head.path),
			head.offset,
		);
	}

	/** Select the whole document. */
	selectAll(): void {
		this.#wasm.selectAll();
	}

	/** Replace the selection with `text`, without input rules. */
	insertText(text: string): boolean {
		return this.#wasm.insertText(text);
	}

	/** Type `text` one character at a time, so input rules such as `## ` apply. */
	typeText(text: string): boolean {
		return this.#wasm.typeText(text);
	}

	/** Delete the selection. */
	deleteSelection(): boolean {
		return this.#wasm.deleteSelection();
	}

	/** Press a key binding such as `Enter`, `Tab`, or `Mod-b` (`Mod` is Ctrl). */
	press(key: string): boolean {
		return this.#wasm.press(key);
	}

	/** Paste `content` read as `format` over the selection. */
	paste(format: Format, content: string): boolean {
		return this.#wasm.paste(format, content);
	}

	/** Undo the last call that changed the document. */
	undo(): boolean {
		return this.#wasm.undo();
	}

	/** Redo the last undone call. */
	redo(): boolean {
		return this.#wasm.redo();
	}

	/** The content and selection as snapshot JSON text, for {@link Engine.restore}. */
	snapshot(): string {
		return this.#wasm.snapshot();
	}

	/** Release the document's memory now rather than when it is collected. */
	[Symbol.dispose](): void {
		this.#wasm.free();
	}
}
