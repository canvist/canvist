/**
 * Every extension canvist ships, collaborating, in a second WebAssembly
 * module, so `@canvist/collab` stays small with only the StarterKit. The API
 * is the same; `createEngine` here also takes the extensions beyond the
 * StarterKit.
 *
 * ```ts
 * import { createEngine, highlight, starterKit } from "@canvist/collab/full";
 *
 * const engine = await createEngine([...starterKit, highlight]);
 * using doc = engine.parse("markdown", "Hello world");
 * doc.share(1);
 * doc.selectAll();
 * doc.commands.toggleHighlight();
 * doc.takeUpdates(); // the change, for the other peers
 * ```
 *
 * The emoji table is data rather than code: the first engine with `emoji`
 * loads it.
 *
 * @module
 */
import {
	bundled,
	type EngineOptions,
	type Extensions,
	type StarterKit,
	type WasmSource,
} from "@canvist/core";
import { starterKit } from "@canvist/core/extensions";
import init, {
	loadEmoji,
	WasmEngine,
} from "../wasm-full/canvist_collab_full.js";
import { CollabEngine } from "./collab.ts";

export {
	type CanvistError,
	Doc,
	Engine,
	type EngineOptions,
	isCanvistError,
	type WasmSource,
} from "@canvist/core";
export type * from "@canvist/core";
export * from "@canvist/core/extensions";
export * from "@canvist/core/extras";
export {
	CollabDoc,
	CollabEngine,
	type SyncReply,
	type Validation,
} from "./collab.ts";

const BUNDLED = new URL(
	"../wasm-full/canvist_collab_full_bg.wasm",
	import.meta.url,
);

let loading: Promise<void> | undefined;
let loadingEmoji: Promise<void> | undefined;

/**
 * Load the full WebAssembly module once. Later calls return the first
 * call's promise. Without a source, Node, Deno, and Bun read the bundled
 * file; runtimes without a file system pass the module they imported.
 */
export function load(source?: WasmSource): Promise<void> {
	loading ??= (async () => {
		await init({ module_or_path: source ?? await bundled(BUNDLED) });
	})();
	return loading;
}

/** Give the module the emoji table, once. */
function emojiTable(): Promise<void> {
	loadingEmoji ??= import("@canvist/core/gemoji").then(({ gemoji }) => {
		loadEmoji(gemoji);
	});
	return loadingEmoji;
}

/**
 * Load canvist and create a collaborating engine with the given
 * extensions, the StarterKit by default. The engine's documents only offer
 * the registered extensions' commands, queries, blocks, and marks, so using
 * another one is a type error.
 */
export async function createEngine<const E extends Extensions = StarterKit>(
	extensions?: E,
	options: EngineOptions = {},
): Promise<CollabEngine<E>> {
	await load(options.wasm);
	const registered = (extensions ?? starterKit) as E;
	const names = registered.map((extension) => extension.name);

	if (names.includes("emoji")) {
		await emojiTable();
	}

	return new CollabEngine(
		new WasmEngine(names, options.client ?? 1),
		registered,
	);
}
