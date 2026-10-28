import { compile, errorLine, evaluate } from "./compile.ts";
import { formatArgs } from "./inspect.ts";
import type { LogLevel, RealmEvent, RunError } from "./protocol.ts";

/**
 * What the server realm and browser realms share: a `console` that reports
 * to the page, and running a file with its imports answered from the
 * realm's own module table.
 */

export type Emit = (event: RealmEvent) => void;

/** A `console` that reports each line to the page, and still logs to devtools. */
export function capturedConsole(emit: Emit, native: Console): Console {
	let indent = "";
	const counts = new Map<string, number>();
	const timers = new Map<string, number>();
	const line = (level: LogLevel) => (...args: unknown[]) => {
		(native[level] as (...args: unknown[]) => void)(...args);
		const text = formatArgs(args);
		emit({
			type: "log",
			level,
			text: indent ? text.replace(/^/gm, indent) : text,
		});
	};
	const log = line("log");
	const captured: Partial<Console> = {
		log,
		info: line("info"),
		warn: line("warn"),
		error: line("error"),
		debug: line("debug"),
		dir: (value: unknown) => log(value),
		trace: line("log"),
		table: (value: unknown) => log(value),
		assert: (condition?: boolean, ...args: unknown[]) => {
			if (!condition) line("error")("Assertion failed", ...args);
		},
		count: (label = "default") => {
			const count = (counts.get(label) ?? 0) + 1;
			counts.set(label, count);
			log(`${label}: ${count}`);
		},
		countReset: (label = "default") => void counts.delete(label),
		time: (label = "default") => void timers.set(label, performance.now()),
		timeLog: (label = "default") => log(`${label}: ${elapsed(timers, label)}`),
		timeEnd: (label = "default") => {
			log(`${label}: ${elapsed(timers, label)}`);
			timers.delete(label);
		},
		group: (...args: unknown[]) => {
			if (args.length > 0) log(...args);
			indent += "  ";
		},
		groupCollapsed: (...args: unknown[]) => {
			if (args.length > 0) log(...args);
			indent += "  ";
		},
		groupEnd: () => {
			indent = indent.slice(2);
		},
	};
	return { ...native, ...captured } as Console;
}

function elapsed(timers: Map<string, number>, label: string): string {
	const start = timers.get(label);
	return start === undefined
		? "no such timer"
		: `${(performance.now() - start).toFixed(2)}ms`;
}

export function runError(error: unknown, file: string): RunError {
	const message = error instanceof Error
		? `${error.name}: ${error.message}`
		: `Uncaught ${formatArgs([error])}`;
	const line = errorLine(error, file);
	return line === undefined ? { message } : { message, line };
}

export interface RunOptions {
	readonly file: string;
	readonly source: string;
	readonly console: Console;
	/** Loads the module `specifier` names, or `undefined` when the realm hasn't got it. */
	readonly load: (specifier: string) => Promise<unknown> | undefined;
	/** The specifiers the realm has, for the error when code imports another. */
	readonly available: readonly string[];
}

/** Runs a file, resolving when its top level finishes and rejecting with what it threw. */
export async function runFile(options: RunOptions): Promise<void> {
	const compiled = compile(options.source);
	const modules = new Map<string, unknown>();
	await Promise.all(
		compiled.requires.map(async (specifier) => {
			const loading = options.load(specifier);
			if (loading === undefined) {
				throw new Error(
					`Cannot find module "${specifier}" in the sandbox. It has ${
						options.available.map((name) => `"${name}"`).join(", ")
					}.`,
				);
			}
			// Marked as ES modules, so default imports read `default` as they would natively.
			modules.set(specifier, {
				__esModule: true,
				...(await loading as object),
			});
		}),
	);
	await evaluate(compiled, options.file, {
		console: options.console,
		require: (specifier) => {
			if (!modules.has(specifier)) {
				throw new Error(`Cannot find module "${specifier}" in the sandbox`);
			}
			return modules.get(specifier);
		},
	});
}
