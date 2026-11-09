/**
 * The conformance fixtures against the StarterKit and full modules, which
 * have every extension but no Yjs collaboration.
 *
 * @module
 */
import { conformance } from "../../canvist-core/tests/conformance.ts";
import { createEngine as createFullEngine } from "../src/full.ts";
import { createEngine } from "../src/mod.ts";

await conformance({
	createEngine: (extensions) => createEngine(extensions as never),
	createFullEngine,
	features: ["extensions"],
});
