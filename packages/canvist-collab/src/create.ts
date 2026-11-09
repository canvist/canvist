/**
 * Creating collaborating engines from the StarterKit WebAssembly module.
 *
 * @module
 */
import type { EngineOptions, StarterKit } from "@canvist/core";
import { starterKit } from "@canvist/core/extensions";
import { WasmEngine } from "../wasm/canvist_collab.js";
import { CollabEngine } from "./collab.ts";
import { load } from "./load.ts";

/**
 * Load canvist and create a collaborating engine with the given StarterKit
 * extensions, all of them by default. Every other extension is in
 * `@canvist/collab/full`.
 *
 * ```ts
 * const engine = await createEngine();
 * using server = engine.parse("markdown", "Hello");
 * server.share(1);
 * using peer = engine.fromYjs(server.encodeState(), 2);
 * ```
 */
export async function createEngine<
	const E extends readonly StarterKit[number][] = StarterKit,
>(extensions?: E, options: EngineOptions = {}): Promise<CollabEngine<E>> {
	await load(options.wasm);
	const registered = (extensions ?? starterKit) as E;
	const names = registered.map((extension) => extension.name);
	return new CollabEngine(
		new WasmEngine(names, options.client ?? 1),
		registered,
	);
}
