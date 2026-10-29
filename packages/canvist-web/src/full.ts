/**
 * The editor with every extension canvist ships.
 *
 * `createEditor` here makes an editor from a build of canvist with every
 * extension inside, and the layout and theme for their blocks and marks:
 * tables, callouts, math, media, ink, and the rest. It is a third
 * WebAssembly module, so editors with only the StarterKit never download
 * it. Everything `@canvist/web` exports is here too, and every extension
 * beyond the StarterKit; `createEditor` here takes them all, and `load`
 * loads this module.
 *
 * ```ts
 * import { allExtensions, createEditor } from "@canvist/web/full";
 *
 * const editor = await createEditor(document.querySelector("#editor")!, {
 * 	extensions: allExtensions,
 * 	content: { format: "markdown", input: "| a | b |\n| - | - |\n| 1 | 2 |" },
 * });
 * editor.commands.addRowAfter();
 * ```
 *
 * @module
 */
import type { Extensions, StarterKit } from "@canvist/headless";
import init, { WebEditor } from "../wasm-full/canvist_web.js";
import { Editor, type EditorOptions } from "./editor.ts";
import type { WasmSource } from "./load.ts";
import type { Plugins } from "./plugin.ts";

export * from "@canvist/headless/extras";
export * from "./mod.ts";

let loading: Promise<void> | undefined;

/**
 * Load the full WebAssembly module once. Without a source it is fetched
 * from next to this file, which bundlers such as Vite follow.
 */
export function load(source?: WasmSource): Promise<void> {
	loading ??= init({
		module_or_path: source ??
			new URL("../wasm-full/canvist_web_bg.wasm", import.meta.url),
	}).then(() => undefined);

	return loading;
}

/**
 * Create an editor that can use any extension: `@canvist/web`'s
 * `createEditor`, with every extension inside. `options.wasm` is where the
 * full module comes from.
 */
export async function createEditor<
	const E extends Extensions = StarterKit,
	const P extends Plugins = [],
>(
	element: HTMLElement,
	options: EditorOptions<E, P> = {},
): Promise<Editor<E, P>> {
	await load(options.wasm);

	return new Editor(element, options, (...args) => new WebEditor(...args));
}
