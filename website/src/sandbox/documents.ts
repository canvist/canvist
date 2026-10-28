import type { Doc, Engine, Extensions } from "@canvist/headless";
import type { DocSnapshot } from "./protocol.ts";

/**
 * Finds "the document the code made", for the preview: every document an
 * engine creates is recorded, and calling any of its methods marks it the
 * most recent. Snapshots go out a moment after the last change, so a sync
 * server's room previews live while people type into it.
 */

type AnyDoc = Doc<Extensions>;
type AnyEngine = Engine<Extensions>;

interface Tracked {
	readonly id: number;
	readonly doc: AnyDoc;
	readonly extensions: readonly string[];
	touched: number;
}

const MAKERS = ["create", "parse", "fromJson", "fromYjs", "restore"] as const;
const SNAPSHOT_DELAY_MS = 60;

export class DocumentTracker {
	readonly #tracked = new Map<AnyDoc, Tracked>();
	readonly #onChange: (documents: readonly DocSnapshot[]) => void;
	/** The unwrapped methods, so taking a snapshot doesn't count as a change. */
	readonly #original = new Map<
		string,
		(this: AnyDoc, ...args: never[]) => unknown
	>();
	#clock = 0;
	#nextId = 1;
	#timer: ReturnType<typeof setTimeout> | undefined;

	constructor(onChange: (documents: readonly DocSnapshot[]) => void) {
		this.#onChange = onChange;
	}

	/** Wraps the classes `@canvist/headless` and `@canvist/headless/full` share, once. */
	watch(
		classes: {
			readonly Engine: { prototype: AnyEngine };
			readonly Doc: { prototype: AnyDoc };
		},
	): void {
		const engine = classes.Engine.prototype as unknown as Record<
			string,
			unknown
		>;
		if (engine["__sandboxTracked"]) return;
		engine["__sandboxTracked"] = true;
		const tracker = this;

		for (const name of MAKERS) {
			const make = engine[name] as (
				this: AnyEngine,
				...args: never[]
			) => AnyDoc;
			engine[name] = function (this: AnyEngine, ...args: never[]) {
				const doc = make.apply(this, args);
				tracker.#record(
					doc,
					this.extensions.map((extension) => extension.name),
				);
				return doc;
			};
		}

		const doc = classes.Doc.prototype as unknown as Record<string, unknown>;
		for (const name of Object.getOwnPropertyNames(classes.Doc.prototype)) {
			const descriptor = Object.getOwnPropertyDescriptor(
				classes.Doc.prototype,
				name,
			);
			if (name === "constructor" || typeof descriptor?.value !== "function") {
				continue;
			}
			const method = descriptor.value as (
				this: AnyDoc,
				...args: never[]
			) => unknown;
			this.#original.set(name, method);
			doc[name] = function (this: AnyDoc, ...args: never[]) {
				const result = method.apply(this, args);
				tracker.#touch(this);
				return result;
			};
		}
	}

	/** Snapshots now, rather than after the delay: the code's top level just finished. */
	flush(): void {
		clearTimeout(this.#timer);
		this.#timer = undefined;
		if (this.#tracked.size > 0) this.#onChange(this.#snapshots());
	}

	#record(doc: AnyDoc, extensions: readonly string[]): void {
		this.#tracked.set(doc, {
			id: this.#nextId++,
			doc,
			extensions,
			touched: ++this.#clock,
		});
		this.#schedule();
	}

	#touch(doc: AnyDoc): void {
		const tracked = this.#tracked.get(doc);
		if (tracked === undefined) return;
		tracked.touched = ++this.#clock;
		this.#schedule();
	}

	#schedule(): void {
		this.#timer ??= setTimeout(() => this.flush(), SNAPSHOT_DELAY_MS);
	}

	/** The documents, most recently used first. A disposed one drops out. */
	#snapshots(): DocSnapshot[] {
		const call = (doc: AnyDoc, name: string, ...args: unknown[]) =>
			(this.#original.get(name) as (
				this: AnyDoc,
				...args: unknown[]
			) => unknown).apply(doc, args);
		const snapshots: { touched: number; snapshot: DocSnapshot }[] = [];
		for (const tracked of this.#tracked.values()) {
			try {
				snapshots.push({
					touched: tracked.touched,
					snapshot: {
						id: tracked.id,
						extensions: tracked.extensions,
						json: call(tracked.doc, "toJson"),
						html: call(tracked.doc, "export", "html") as string,
						markdown: call(tracked.doc, "export", "markdown") as string,
					},
				});
			} catch {
				// `using` or `[Symbol.dispose]()` freed it; there's nothing to show.
				this.#tracked.delete(tracked.doc);
			}
		}
		return snapshots.sort((a, b) => b.touched - a.touched).map(({ snapshot }) =>
			snapshot
		);
	}
}
