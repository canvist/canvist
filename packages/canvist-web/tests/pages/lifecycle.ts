/**
 * Editors the lifecycle tests create and destroy themselves:
 * `window.canvist` has `createEditor` and the fonts plugin, once the
 * WebAssembly module has loaded.
 */
import { fonts } from "../../src/fonts.ts";
import { createEditor, load } from "../../src/mod.ts";

await load();

Object.assign(globalThis, { canvist: { createEditor, fonts } });
