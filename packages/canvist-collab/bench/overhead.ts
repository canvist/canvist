/**
 * What a collaborator's keystroke costs through the WebAssembly boundary,
 * measured on the same operation as
 * `crates/canvist_headless/tests/perf_budgets.rs`, so the difference is the
 * binding's overhead, and the collaborating modules' sizes.
 *
 * ```sh
 * node bench/overhead.ts           # print the numbers
 * node bench/overhead.ts --check   # also fail when one is over budget
 * ```
 *
 * `CANVIST_PERF_SLACK` multiplies every budget, as it does for the Rust
 * budgets.
 *
 * @module
 */
import { readFileSync } from "node:fs";

import { createEngine } from "../src/mod.ts";

const PARAGRAPH =
	"The quick brown fox jumps over the lazy dog, again and again, until the line wraps.";
const PARAGRAPHS = 10_000;
const SAMPLES = 15;
const CARET = { path: [PARAGRAPHS / 2], offset: 10 };
const WASM = new URL("../wasm/canvist_collab_bg.wasm", import.meta.url);
const WASM_FULL = new URL(
	"../wasm-full/canvist_collab_full_bg.wasm",
	import.meta.url,
);

const slack = Number(process.env.CANVIST_PERF_SLACK ?? 1);
const check = process.argv.includes("--check");
const failures: string[] = [];

/** The median time of one call, in microseconds, over `SAMPLES` batches. */
function perCall(batch: number, run: () => unknown): number {
	run();
	const samples = Array.from({ length: SAMPLES }, () => {
		const start = performance.now();

		for (let index = 0; index < batch; index += 1) {
			run();
		}

		return ((performance.now() - start) * 1000) / batch;
	});

	return samples.sort((a, b) => a - b)[Math.floor(SAMPLES / 2)] ?? Number.NaN;
}

function report(
	name: string,
	measured: number,
	budget: number,
	unit: "µs" | "ms" | "KB",
): void {
	const allowed = budget * slack;
	const over = measured > allowed;
	console.log(
		`${name}: ${
			measured.toFixed(unit === "µs" ? 2 : 1)
		} ${unit} (budget ${budget} ${unit})${over ? "  OVER" : ""}`,
	);

	if (over) {
		failures.push(name);
	}
}

const engine = await createEngine();
const markdown = Array.from(
	{ length: PARAGRAPHS },
	(_, index) => `${PARAGRAPH} ${index}`,
).join("\n\n");

const server = engine.parse("markdown", markdown);
server.share(1);
const peer = engine.fromYjs(server.encodeState(), 2);
const updates: Uint8Array[] = [];

for (let index = 0; index < (SAMPLES + 1) * 200; index += 1) {
	peer.select(CARET);
	peer.insertText("y");
	peer.press("Backspace");
	updates.push(...peer.takeUpdates());
}

let next = 0;
report(
	"apply one Yjs keystroke update @10k paragraphs",
	perCall(200, () => server.applyUpdate(updates[next++] ?? new Uint8Array())),
	30,
	"µs",
);

// The StarterKit module with Yjs, which `@canvist/collab` loads.
report(
	"WebAssembly module (wasm-opt, raw)",
	readFileSync(WASM).byteLength / 1024,
	1_200,
	"KB",
);
// Every extension canvist ships, with Yjs, for `@canvist/collab/full`.
report(
	"full WebAssembly module (wasm-opt, raw)",
	readFileSync(WASM_FULL).byteLength / 1024,
	1_400,
	"KB",
);

if (check && failures.length > 0) {
	console.error(`over budget: ${failures.join(", ")}`);
	process.exit(1);
}
