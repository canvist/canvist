import { assert, assertEquals } from "@std/assert";
import {
	allowsExtras,
	type Catalog,
	mask,
	readExtensionList,
	writeExtensionList,
} from "../../src/sandbox/extension-list.ts";

const catalog: Catalog = {
	starterKit: ["heading", "bold", "italic"],
	extras: ["highlight", "table"],
};

function read(code: string) {
	const result = readExtensionList(code, catalog);
	if (!result.ok) throw new Error(result.reason);
	return result.list;
}

function toggle(code: string, names: string[]): string {
	return writeExtensionList(code, read(code), names, catalog);
}

Deno.test("reads the list passed to createEngine, expanding spreads", () => {
	const code =
		`import { createEngine, highlight, starterKit } from "@canvist/headless/full";\n\nconst engine = await createEngine([...starterKit, highlight]);\n`;
	const list = read(code);
	assertEquals(list.names, ["heading", "bold", "italic", "highlight"]);
	assertEquals(list.module, "@canvist/headless/full");
	assertEquals(
		code.slice(list.open, list.close + 1),
		"[...starterKit, highlight]",
	);
});

Deno.test("reads an editor's `extensions` option", () => {
	const code =
		`import { bold, createEditor } from "@canvist/web";\nconst editor = await createEditor(element, { extensions: [bold], content });\n`;
	assertEquals(read(code).names, ["bold"]);
	assertEquals(allowsExtras(read(code).module), false);
});

Deno.test("brackets and names in strings and comments don't count", () => {
	const code =
		`import { bold, createEngine } from "@canvist/headless";\n// createEngine([italic])\nconst engine = await createEngine([bold /* ] */]);\nconsole.log("extensions: [heading]");\n`;
	assertEquals(read(code).names, ["bold"]);
	assertEquals(mask(`a("b]")`).length, `a("b]")`.length);
});

Deno.test("a list with anything but extension names is left alone", () => {
	const code =
		`import { createEngine } from "@canvist/headless";\nconst engine = await createEngine([bold.configure()]);\n`;
	const result = readExtensionList(code, catalog);
	assert(!result.ok);
});

Deno.test("code without a list says how to add one", () => {
	const result = readExtensionList(
		`import { createEngine } from "@canvist/headless";\nconst engine = await createEngine();\n`,
		catalog,
	);
	assert(!result.ok && result.reason.includes("extensions"));
});

Deno.test("switching an extension on adds it to the list and the import", () => {
	const code =
		`import { bold, createEngine } from "@canvist/headless";\n\nconst engine = await createEngine([bold]);\n`;
	assertEquals(
		toggle(code, ["bold", "italic"]),
		`import { bold, createEngine, italic } from "@canvist/headless";\n\nconst engine = await createEngine([bold, italic]);\n`,
	);
});

Deno.test("switching one off removes its import unless the code still uses it", () => {
	const code =
		`import { bold, createEngine, italic } from "@canvist/headless";\n\nconst engine = await createEngine([bold, italic]);\nconsole.log(italic.name);\n`;
	assertEquals(
		toggle(code, ["bold"]),
		`import { bold, createEngine, italic } from "@canvist/headless";\n\nconst engine = await createEngine([bold]);\nconsole.log(italic.name);\n`,
	);
	const unused =
		`import { bold, createEngine, italic } from "@canvist/headless";\nconst engine = await createEngine([bold, italic]);\n`;
	assertEquals(
		toggle(unused, ["bold"]),
		`import { bold, createEngine } from "@canvist/headless";\nconst engine = await createEngine([bold]);\n`,
	);
});

Deno.test("the whole StarterKit becomes a spread, and extras move the import to /full", () => {
	const code =
		`import { bold, createEngine } from "@canvist/headless";\nconst engine = await createEngine([bold]);\n`;
	assertEquals(
		toggle(code, ["heading", "bold", "italic", "table"]),
		`import { createEngine, starterKit, table } from "@canvist/headless/full";\nconst engine = await createEngine([...starterKit, table]);\n`,
	);
	assertEquals(
		toggle(code, [...catalog.starterKit, ...catalog.extras]),
		`import { allExtensions, createEngine } from "@canvist/headless/full";\nconst engine = await createEngine([...allExtensions]);\n`,
	);
});

Deno.test("long lists wrap one per line at the list's indent", () => {
	const wide: Catalog = {
		starterKit: [
			"heading",
			"blockquote",
			"codeBlock",
			"bulletList",
			"orderedList",
			"bold",
			"italic",
			"strike",
		],
		extras: [],
	};
	const code =
		`import { bold, createEditor } from "@canvist/web";\n\nconst editor = await createEditor(element, {\n\textensions: [bold],\n});\n`;
	const result = readExtensionList(code, wide);
	if (!result.ok) throw new Error(result.reason);
	const written = writeExtensionList(code, result.list, [
		"heading",
		"blockquote",
		"codeBlock",
		"bulletList",
		"orderedList",
		"bold",
		"italic",
	], wide);
	assertEquals(
		written,
		`import {\n\tblockquote,\n\tbold,\n\tbulletList,\n\tcodeBlock,\n\tcreateEditor,\n\theading,\n\titalic,\n\torderedList,\n} from "@canvist/web";\n\nconst editor = await createEditor(element, {\n\textensions: [\n\t\theading,\n\t\tblockquote,\n\t\tcodeBlock,\n\t\tbulletList,\n\t\torderedList,\n\t\tbold,\n\t\titalic,\n\t],\n});\n`,
	);
	// And the wrapped list reads back the same.
	const again = readExtensionList(written, wide);
	assert(again.ok);
	assertEquals(again.list.names, [
		"heading",
		"blockquote",
		"codeBlock",
		"bulletList",
		"orderedList",
		"bold",
		"italic",
	]);
});
