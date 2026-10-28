import { transform } from "sucrase";

/**
 * TypeScript to something a realm can run: types stripped, and `import`s
 * turned into `require` calls that the realm answers from its own module
 * table, the way Node runs a CommonJS module. Top-level `await` keeps
 * working because the realm runs the result inside an async function.
 *
 * Sucrase keeps every statement on its own line, so line numbers in errors
 * point at the code as written.
 */
export interface Compiled {
	/** The function body to run. */
	readonly code: string;
	/** Every module the code requires, to load before it runs. */
	readonly requires: readonly string[];
}

const REQUIRE = /\brequire\((["'])([^"']+)\1\)/g;

export function compile(source: string): Compiled {
	const { code } = transform(source, {
		transforms: ["typescript", "imports"],
		disableESTransforms: true,
		production: true,
	});
	const requires = [
		...new Set([...code.matchAll(REQUIRE)].map((match) => match[2] ?? "")),
	];
	return { code, requires };
}

/** The bindings a module body sees, as Node gives a CommonJS module. */
export interface ModuleScope {
	readonly require: (specifier: string) => unknown;
	readonly console: Console;
}

type ModuleBody = (
	require: ModuleScope["require"],
	exports: Record<string, unknown>,
	module: { exports: Record<string, unknown> },
	console: Console,
) => Promise<void>;

/**
 * Runs compiled code as a module named `file`, resolving when its
 * top-level code (including any `await`s) has finished.
 *
 * The wrapper opens on the code's first line, so the engine's line numbers
 * are the source's, and `sourceURL` names the file in stack traces.
 */
export function evaluate(
	compiled: Compiled,
	file: string,
	scope: ModuleScope,
): Promise<void> {
	const wrapped =
		`(async function (require, exports, module, console) {${compiled.code}\n})\n//# sourceURL=${file}`;
	// Indirect eval runs in the realm's global scope, not this module's.
	const body = (0, eval)(wrapped) as ModuleBody;
	const module = { exports: {} };
	return body(scope.require, module.exports, module, scope.console);
}

/** Where an error came from in `file`, from its stack. */
export function errorLine(error: unknown, file: string): number | undefined {
	if (!(error instanceof Error) || error.stack === undefined) return undefined;
	const escaped = file.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
	// Browsers write `sample.ts:4:7`; Deno writes `sample.ts, <anonymous>:4:7`.
	const match = new RegExp(`${escaped}(?:, <anonymous>)?:(\\d+)`).exec(
		error.stack,
	);
	return match ? Number(match[1]) : undefined;
}
