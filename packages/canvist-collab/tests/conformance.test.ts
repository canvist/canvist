/**
 * The conformance fixtures against the collaborating modules, which run
 * every case: the `collab` ones too.
 *
 * @module
 */
import { conformance } from "../../canvist-core/tests/conformance.ts";
import { createEngine as createFullEngine } from "../src/full.ts";
import { createEngine } from "../src/mod.ts";

await conformance({
	createEngine: (extensions) => createEngine(extensions as never),
	createFullEngine: (extensions) => createFullEngine(extensions),
	features: ["collab", "extensions"],
});
