/**
 * `console.log` formatting, close to Deno's, so a sample prints in the
 * browser what the docs say it prints (they're checked under Deno):
 * `{ rejectedBlocks: 0, waiting: false }`, `[ "heading", "paragraph" ]`.
 */

const MAX_DEPTH = 4;
const LINE_WIDTH = 72;
const IDENTIFIER = /^[A-Za-z_$][\w$]*$/;

/** One `console.log` call's arguments as the line it prints. */
export function formatArgs(args: readonly unknown[]): string {
	return args.map((arg) => (typeof arg === "string" ? arg : inspect(arg))).join(
		" ",
	);
}

export function inspect(value: unknown): string {
	return format(value, 0, new Set());
}

function format(value: unknown, depth: number, seen: Set<object>): string {
	switch (typeof value) {
		case "string":
			return JSON.stringify(value);
		case "number":
			return Object.is(value, -0) ? "-0" : String(value);
		case "bigint":
			return `${value}n`;
		case "boolean":
		case "undefined":
			return String(value);
		case "symbol":
			return value.toString();
		case "function":
			return /^class\b/.test(Function.prototype.toString.call(value))
				? `[class ${value.name || "(anonymous)"}]`
				: `[Function: ${value.name || "(anonymous)"}]`;
	}
	if (value === null) return "null";
	const object = value as object;
	if (seen.has(object)) return "[Circular]";
	if (object instanceof Error) {
		return object.stack ?? `${object.name}: ${object.message}`;
	}
	if (object instanceof Date) return object.toISOString();
	if (object instanceof RegExp) return object.toString();

	seen.add(object);
	try {
		return formatObject(object, depth, seen);
	} finally {
		seen.delete(object);
	}
}

function formatObject(
	object: object,
	depth: number,
	seen: Set<object>,
): string {
	const nested = (child: unknown) => format(child, depth + 1, seen);

	if (Array.isArray(object)) {
		if (depth >= MAX_DEPTH) return "[Array]";
		return wrap("", "[", object.map(nested), "]", depth);
	}
	if (ArrayBuffer.isView(object) && !(object instanceof DataView)) {
		const items = Array.from(
			object as unknown as ArrayLike<number | bigint>,
			(item) => format(item, depth + 1, seen),
		);
		const name = object.constructor.name;
		return wrap(`${name}(${items.length}) `, "[", items, "]", depth);
	}
	if (object instanceof Map) {
		if (depth >= MAX_DEPTH) return "[Map]";
		const items = [...object].map(([key, item]) =>
			`${nested(key)} => ${nested(item)}`
		);
		return wrap(`Map(${object.size}) `, "{", items, "}", depth);
	}
	if (object instanceof Set) {
		if (depth >= MAX_DEPTH) return "[Set]";
		return wrap(
			`Set(${object.size}) `,
			"{",
			[...object].map(nested),
			"}",
			depth,
		);
	}

	const prototype: unknown = Object.getPrototypeOf(object);
	const name = prototype === null || prototype === Object.prototype
		? ""
		: (prototype as { constructor?: { name?: string } }).constructor?.name ??
			"";
	if (depth >= MAX_DEPTH) return name ? `[${name}]` : "[Object]";
	const items = Object.entries(object).map(([key, item]) =>
		`${IDENTIFIER.test(key) ? key : JSON.stringify(key)}: ${nested(item)}`
	);
	return wrap(name ? `${name} ` : "", "{", items, "}", depth);
}

/**
 * Entries on one line when they fit, otherwise one per line. Nested values
 * come back unindented, and each level indents its own entries.
 */
function wrap(
	prefix: string,
	open: string,
	items: readonly string[],
	close: string,
	depth: number,
): string {
	if (items.length === 0) return `${prefix}${open}${close}`;
	const line = `${prefix}${open} ${items.join(", ")} ${close}`;
	if (line.length + depth * 2 <= LINE_WIDTH && !line.includes("\n")) {
		return line;
	}
	const body = items.map((item) => `  ${item.replaceAll("\n", "\n  ")}`).join(
		",\n",
	);
	return `${prefix}${open}\n${body}\n${close}`;
}
