/**
 * Reads and rewrites the extension list in a file of canvist code, so the
 * playground's extension switches and the code stay one thing: switching
 * an extension edits the code, and editing the code moves the switches.
 *
 * The list is the first `createEngine([...])` argument or
 * `extensions: [...]` option. It may name extensions and spread the
 * groups (`...starterKit`); anything else in it, and the switches leave the
 * code alone. The import the factory comes from gains and loses the
 * extensions the list names, and moves to the module's `/full` entry
 * (`@canvist/headless/full`, `@canvist/web/full`) when the list needs
 * extensions beyond the StarterKit.
 */

export interface Catalog {
	/** Extensions in the order lists name them: the StarterKit's, then the rest. */
	readonly starterKit: readonly string[];
	readonly extras: readonly string[];
}

export interface ExtensionList {
	/** The offsets of the list's brackets in the code. */
	readonly open: number;
	readonly close: number;
	/** The extensions the list registers, spreads expanded. */
	readonly names: readonly string[];
	/** The module the file's `createEngine` or `createEditor` comes from. */
	readonly module: string;
}

export type ReadResult =
	| { readonly ok: true; readonly list: ExtensionList }
	| { readonly ok: false; readonly reason: string };

/**
 * Where the factory and the extensions come from in the same import.
 * `@canvist/web/collab` exports no extensions, so collaborative editors
 * aren't switched here.
 */
const FACTORY_MODULES = [
	"@canvist/headless",
	"@canvist/headless/full",
	"@canvist/web",
	"@canvist/web/full",
];
const LINE_WIDTH = 80;

export function readExtensionList(code: string, catalog: Catalog): ReadResult {
	const masked = mask(code);
	const factory = factoryImport(code, masked);
	if (factory === undefined) {
		return {
			ok: false,
			reason:
				"The switches edit code that imports createEngine from @canvist/headless, or createEditor from @canvist/web.",
		};
	}

	const opening = /\bcreateEngine\s*\(\s*\[|\bextensions\s*:\s*\[/g.exec(
		masked,
	);
	if (opening === null) {
		return {
			ok: false,
			reason:
				"Add an `extensions: [...]` list, or pass one to createEngine, to switch extensions.",
		};
	}
	const open = opening.index + opening[0].length - 1;
	const close = matching(masked, open);
	if (close === undefined) {
		return { ok: false, reason: "The extension list isn't closed." };
	}

	const groups = groupsOf(catalog);
	const known = new Set([...catalog.starterKit, ...catalog.extras]);
	const names: string[] = [];
	for (const item of splitTopLevel(masked, open + 1, close)) {
		// Comments in the list don't count; rewriting it drops them.
		const text = masked.slice(item.start, item.end).trim();
		if (text === "") continue;
		const spread = /^\.\.\.\s*([A-Za-z_$][\w$]*)$/.exec(text);
		const group = spread ? groups.get(spread[1] ?? "") : undefined;
		if (group) names.push(...group);
		else if (!spread && known.has(text)) names.push(text);
		else {return {
				ok: false,
				reason:
					`The switches only follow lists of extension names, and this one has \`${
						code.slice(item.start, item.end).trim()
					}\`.`,
			};}
	}
	return {
		ok: true,
		list: { open, close, names: [...new Set(names)], module: factory.module },
	};
}

/** `code` with its extension list registering exactly `names`, and its import to match. */
export function writeExtensionList(
	code: string,
	list: ExtensionList,
	names: Iterable<string>,
	catalog: Catalog,
): string {
	const chosen = new Set(names);
	const starterKit = catalog.starterKit.filter((name) => chosen.has(name));
	const extras = catalog.extras.filter((name) => chosen.has(name));
	const items = starterKit.length === catalog.starterKit.length
		? extras.length === catalog.extras.length
			? ["...allExtensions"]
			: ["...starterKit", ...extras]
		: [...starterKit, ...extras];

	const lineStart = code.lastIndexOf("\n", list.open) + 1;
	const indent = /^[\t ]*/.exec(code.slice(lineStart))?.[0] ?? "";
	const lineEnd = code.indexOf("\n", list.close);
	const after = code.slice(
		list.close + 1,
		lineEnd === -1 ? code.length : lineEnd,
	);
	const oneLine = `[${items.join(", ")}]`;
	const fits =
		width(code.slice(lineStart, list.open)) + oneLine.length + after.length <=
			LINE_WIDTH;
	const written = fits || items.length === 0
		? oneLine
		: `[\n${items.map((item) => `${indent}\t${item},`).join("\n")}\n${indent}]`;
	const withList = code.slice(0, list.open) + written +
		code.slice(list.close + 1);

	const needs = items.map((item) => item.replace(/^\.\.\./, ""));
	const module = extras.length > 0 && !list.module.endsWith("/full")
		? `${list.module}/full`
		: list.module;
	return rewriteImport(withList, catalog, needs, module);
}

/** Every extension name and group name a file may import. */
function importable(catalog: Catalog): Set<string> {
	return new Set([
		...catalog.starterKit,
		...catalog.extras,
		...groupsOf(catalog).keys(),
	]);
}

function groupsOf(catalog: Catalog): Map<string, readonly string[]> {
	return new Map([
		["starterKit", catalog.starterKit],
		["extras", catalog.extras],
		["allExtensions", [...catalog.starterKit, ...catalog.extras]],
	]);
}

interface FactoryImport {
	readonly start: number;
	readonly end: number;
	readonly module: string;
	readonly specifiers: readonly string[];
}

const IMPORT = /import\s*\{([^}]*)\}\s*from\s*(["'])([^"']+)\2\s*;?/g;

function factoryImport(
	code: string,
	masked: string,
): FactoryImport | undefined {
	// The module name is a string, which masking blanks, so it's read from the code.
	for (
		const match of masked.matchAll(
			/import\s*\{[^}]*\}\s*from\s*["'][^"']*["']\s*;?/g,
		)
	) {
		const statement = code.slice(match.index, match.index + match[0].length);
		const parsed = new RegExp(IMPORT.source).exec(statement);
		if (parsed === null) continue;
		const module = parsed[3] ?? "";
		const specifiers = (parsed[1] ?? "").split(",").map((specifier) =>
			specifier.trim()
		).filter(Boolean);
		if (!FACTORY_MODULES.includes(module)) continue;
		if (
			!specifiers.some((specifier) =>
				/^(createEngine|createEditor)\b/.test(specifier)
			)
		) continue;
		return {
			start: match.index,
			end: match.index + match[0].length,
			module,
			specifiers,
		};
	}
	return undefined;
}

function rewriteImport(
	code: string,
	catalog: Catalog,
	needs: readonly string[],
	module: string,
): string {
	const factory = factoryImport(code, mask(code));
	if (factory === undefined) return code;
	const extensionNames = importable(catalog);
	const rest = mask(code.slice(0, factory.start) + code.slice(factory.end));
	const used = (name: string) =>
		new RegExp(`(?<![\\w$.])${name.replace(/\$/g, "\\$")}(?![\\w$])`).test(
			rest,
		);
	const kept = factory.specifiers.filter((specifier) =>
		!extensionNames.has(specifier) || used(specifier)
	);
	const specifiers = [...new Set([...kept, ...needs])].sort((a, b) =>
		sortKey(a).localeCompare(sortKey(b))
	);

	const quote =
		/from\s*(["'])/.exec(code.slice(factory.start, factory.end))?.[1] ?? '"';
	const semicolon =
		code.slice(factory.start, factory.end).trimEnd().endsWith(";") ? ";" : "";
	const oneLine = `import { ${
		specifiers.join(", ")
	} } from ${quote}${module}${quote}${semicolon}`;
	const statement = oneLine.length <= LINE_WIDTH
		? oneLine
		: `import {\n${
			specifiers.map((specifier) => `\t${specifier},`).join("\n")
		}\n} from ${quote}${module}${quote}${semicolon}`;
	return code.slice(0, factory.start) + statement + code.slice(factory.end);
}

function sortKey(specifier: string): string {
	return specifier.replace(/^type\s+/, "").toLowerCase();
}

/** Columns a line prefix takes, with tabs as two, as the editors here show them. */
function width(text: string): number {
	return text.replace(/\t/g, "  ").length;
}

/** The bracket that closes the one at `open`, in masked code. */
function matching(masked: string, open: number): number | undefined {
	let depth = 0;
	for (let index = open; index < masked.length; index++) {
		const char = masked[index];
		if (char === "[" || char === "(" || char === "{") depth++;
		else if (char === "]" || char === ")" || char === "}") {
			depth--;
			if (depth === 0) return index;
		}
	}
	return undefined;
}

/** The comma-separated items between `from` and `to`, ignoring commas in nested brackets. */
function splitTopLevel(
	masked: string,
	from: number,
	to: number,
): { start: number; end: number }[] {
	const items: { start: number; end: number }[] = [];
	let depth = 0;
	let start = from;
	for (let index = from; index < to; index++) {
		const char = masked[index];
		if (char === "[" || char === "(" || char === "{") depth++;
		else if (char === "]" || char === ")" || char === "}") depth--;
		else if (char === "," && depth === 0) {
			items.push({ start, end: index });
			start = index + 1;
		}
	}
	items.push({ start, end: to });
	return items;
}

/**
 * `code` with the insides of comments and strings blanked, keeping every
 * offset, so brackets and names in them don't count.
 */
export function mask(code: string): string {
	let out = "";
	let index = 0;
	while (index < code.length) {
		const char = code[index] ?? "";
		const next = code[index + 1] ?? "";
		let end = index + 1;
		if (char === "/" && next === "/") {
			end = code.indexOf("\n", index);
			if (end === -1) end = code.length;
		} else if (char === "/" && next === "*") {
			end = code.indexOf("*/", index + 2);
			end = end === -1 ? code.length : end + 2;
		} else if (char === '"' || char === "'" || char === "`") {
			end = index + 1;
			while (end < code.length && code[end] !== char) {
				end += code[end] === "\\" ? 2 : 1;
			}
			end = Math.min(end + 1, code.length);
			out += char + code.slice(index + 1, end - 1).replace(/[^\n]/g, " ") +
				(end - 1 > index ? char : "");
			index = end;
			continue;
		} else {
			out += char;
			index++;
			continue;
		}
		out += code.slice(index, end).replace(/[^\n]/g, " ");
		index = end;
	}
	return out;
}
