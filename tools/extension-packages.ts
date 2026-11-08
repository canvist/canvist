/**
 * Write the per-extension npm and Dart packages: `@canvist/extension-<name>`
 * in `packages/extensions/<name>` and `canvist_extension_<name>` in
 * `platforms/dart_extensions/<name>`, one for each Rust crate in
 * `crates/extensions` that adds extensions.
 *
 * The TypeScript and Dart engines compile every extension into one native
 * module, so these packages hold no code of their own: each re-exports its
 * extensions from `@canvist/headless` or `package:canvist`, the way TipTap
 * ships `@tiptap/extension-bold`. The names come from the metadata
 * snapshots `canvist_extensions` tests, and the check fails if an
 * extension there has no package.
 *
 * A Dart package starts with `publish_to: none`, which keeps the release
 * job from publishing it, until its name is reserved on pub.dev and
 * automated publishing is on (`platforms/dart_extensions/readme.md`). The
 * generator keeps whatever the pubspec says.
 *
 * ```sh
 * node tools/extension-packages.ts          # write the packages
 * node tools/extension-packages.ts --check  # fail if any is out of date
 * ```
 *
 * @module
 */
import {
	existsSync,
	mkdirSync,
	readdirSync,
	readFileSync,
	writeFileSync,
} from "node:fs";

const root = new URL("../", import.meta.url);
const snapshots = new URL("crates/canvist_extensions/tests/snapshots/", root);

/** Crates in `crates/extensions` that hold no extension of their own. */
const SUPPORT = ["clipboard", "document", "media", "paragraph", "suggestion"];

/** Extensions every engine registers, so no package offers them. */
const CORE = ["baseKeymap", "doc", "paragraph", "undoInputRule"];

/** Crates whose extensions are not just the crate's name in camelCase. */
const GROUPS: Record<string, readonly string[]> = {
	list: ["bulletList", "orderedList", "listItem", "taskList"],
	text_style: [
		"textStyle",
		"color",
		"backgroundColor",
		"fontFamily",
		"fontSize",
		"lineHeight",
	],
};

const VERSION = readFileSync(new URL("Cargo.toml", root), "utf8").match(
	/\[workspace\.package\]\nversion = "([^"]+)"/,
)![1];

interface Package {
	/** The crate directory, such as `code_block`. */
	readonly crate: string;
	/** The extension names, such as `codeBlock`. */
	readonly names: readonly string[];
	/** What the crate adds, from its Cargo description. */
	readonly description: string;
	/** Whether `@canvist/headless` compiles these in, or only `/full`. */
	readonly starterKit: boolean;
}

function snapshot(name: string): readonly string[] {
	const text = readFileSync(new URL(name, snapshots), "utf8");
	const body = text.replace(/^---\n[\s\S]*?\n---\n/, "");
	return (JSON.parse(body) as { name: string }[]).map((item) => item.name);
}

function camel(snake: string): string {
	return snake.replace(
		/_([a-z])/g,
		(_, letter: string) => letter.toUpperCase(),
	);
}

function pascal(name: string): string {
	return name[0].toUpperCase() + name.slice(1);
}

function kebab(snake: string): string {
	return snake.replaceAll("_", "-");
}

function description(crate: string): string {
	const manifest = readFileSync(
		new URL(`crates/extensions/${crate}/Cargo.toml`, root),
		"utf8",
	);
	const text = manifest.match(/^description = "(.*)"$/m)![1].replace(
		/^canvist: /,
		"",
	);
	return text.replaceAll("\\\\", "\\");
}

function packages(): readonly Package[] {
	const every = snapshot("extensions_api__extension_metadata.snap");
	const starterKit = snapshot("api__starter_kit_metadata.snap");
	const crates = readdirSync(new URL("crates/extensions/", root)).filter((
		crate,
	) =>
		existsSync(new URL(`crates/extensions/${crate}/Cargo.toml`, root)) &&
		!SUPPORT.includes(crate)
	).sort();
	const found = crates.map((crate): Package => {
		const names = GROUPS[crate] ?? [camel(crate)];
		const missing = names.filter((name) => !every.includes(name));

		if (missing.length > 0) {
			throw new Error(
				`crate ${crate} names unknown extensions: ${missing.join(", ")}`,
			);
		}

		return {
			crate,
			names,
			description: description(crate),
			starterKit: names.every((name) => starterKit.includes(name)),
		};
	});
	const covered = found.flatMap((item) => item.names);
	const orphans = every.filter((name) =>
		!CORE.includes(name) && !covered.includes(name)
	);

	if (orphans.length > 0) {
		throw new Error(
			`no package offers ${orphans.join(", ")}; add it to GROUPS`,
		);
	}

	return found;
}

/** `text` in lines of at most 80 columns, each starting with `prefix`. */
function wrap(text: string, prefix: string): string[] {
	const lines: string[] = [];
	let line = prefix;

	for (const word of text.split(" ")) {
		if (line !== prefix && `${line} ${word}`.length > 80) {
			lines.push(line);
			line = prefix;
		}

		line = line === prefix ? `${prefix}${word}` : `${line} ${word}`;
	}

	return [...lines, line];
}

/** An export of `names` from `uri`, laid out as `dart format` lays it out. */
function exportShow(uri: string, names: readonly string[]): string {
	const line = `export '${uri}' show ${names.join(", ")};`;
	const show = `    show ${names.join(", ")};`;

	if (line.length <= 80) {
		return line;
	}

	return show.length <= 80
		? `export '${uri}'\n${show}`
		: `export '${uri}'\n    show\n${
			names.map((name) => `        ${name}`).join(",\n")
		};`;
}

const LICENSE = readFileSync(new URL("license", root), "utf8");
const HEADER = "// Generated by tools/extension-packages.ts. Do not edit.\n";

/** The files of `@canvist/extension-<name>`. */
function npm(item: Package): Record<string, string> {
	const name = `@canvist/extension-${kebab(item.crate)}`;
	const directory = `packages/extensions/${kebab(item.crate)}`;
	const from = item.starterKit
		? "@canvist/headless/extensions"
		: "@canvist/headless/extras";
	const module = item.starterKit
		? "@canvist/headless"
		: "@canvist/headless/full";
	const sorted = [...item.names].sort((a, b) =>
		a.toLowerCase().localeCompare(b.toLowerCase())
	);
	const line = `export { ${sorted.join(", ")} } from "${from}";\n`;
	const exports = line.length <= 81
		? line
		: `export {\n${
			sorted.map((name) => `\t${name},\n`).join("")
		}} from "${from}";\n`;
	const manifest = {
		name,
		version: VERSION,
		description: `${item.description}, for canvist.`,
		license: "Unlicense",
		homepage:
			`https://github.com/canvist/canvist/tree/main/${directory}#readme`,
		repository: {
			type: "git",
			url: "git+https://github.com/canvist/canvist.git",
			directory,
		},
		bugs: {
			url: "https://github.com/canvist/canvist/issues",
			email: "support@canvist.dev",
		},
		keywords: [
			"canvist",
			"editor",
			"rich-text",
			"extension",
			kebab(item.crate),
		],
		type: "module",
		exports: {
			".": {
				types: "./mod.d.ts",
				default: "./mod.js",
			},
		},
		files: ["mod.js", "mod.d.ts"],
		sideEffects: false,
		peerDependencies: {
			"@canvist/headless": `^${VERSION}`,
		},
	};
	const list = item.names.map((extension) => `\`${extension}\``).join(", ");
	const first = item.names[0];
	const readme = `# ${name}

${item.description}, for [canvist](https://github.com/canvist/canvist).

This package exports ${list} from \`${from}\`, so you can import ${
		item.names.length === 1 ? "it" : "them"
	} on ${
		item.names.length === 1 ? "its" : "their"
	} own, as TipTap ships each extension. canvist compiles every extension into its WebAssembly module, so this package adds no code: ${
		item.starterKit
			? "the StarterKit module, `@canvist/headless`, has these extensions."
			: "these extensions are in the full module, `@canvist/headless/full`, so create the engine from there."
	}

\`\`\`sh
npm install @canvist/headless ${name}
\`\`\`

\`\`\`ts
import { ${first} } from "${name}";
import { createEngine } from "${module}";

const engine = await createEngine([${first}]);
\`\`\`

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.

## License

[Unlicense](https://unlicense.org)
`;

	return {
		"package.json": `${JSON.stringify(manifest, null, "\t")}\n`,
		"mod.js": HEADER + exports,
		"mod.d.ts": HEADER + exports,
		"readme.md": readme,
		license: LICENSE,
	};
}

/** The files of the Dart package `canvist_extension_<name>`. */
function dart(
	item: Package,
	existing: string | undefined,
): Record<string, string> {
	const name = `canvist_extension_${item.crate}`;
	const classes = item.names.flatMap((extension) => [
		`${pascal(extension)}Extension`,
		`${pascal(extension)}Handle`,
	]);
	const constants = item.names.map((extension) =>
		`/// The \`${extension}\` extension: register it with [EngineBuilder.add].\nconst ${extension} = Extensions.${extension};\n`
	);
	const library = [
		HEADER,
		...wrap(`${item.description}, for canvist.`, "/// "),
		"///",
		"/// canvist compiles every extension into its engine, so this library",
		"/// adds no code: it re-exports from `package:canvist`, so you can",
		"/// import an extension on its own.",
		"library;",
		"",
		"import 'package:canvist/canvist.dart';",
		"",
		exportShow("package:canvist/canvist.dart", classes),
		"",
		constants.join("\n"),
	].join("\n");
	// Keep `publish_to: none` until the name is reserved on pub.dev.
	const unpublished = existing === undefined ||
		/^publish_to: none$/m.test(existing);
	const summary = `${item.description}, for canvist's Dart engine.`;
	const pubspec = [
		`name: ${name}`,
		`description: ${JSON.stringify(summary)}`,
		`version: ${VERSION}`,
		`repository: https://github.com/canvist/canvist/tree/main/platforms/dart_extensions/${item.crate}`,
		"issue_tracker: https://github.com/canvist/canvist/issues",
		...(unpublished
			? [
				"# Reserved on pub.dev? Remove this line (see ../readme.md).",
				"publish_to: none",
			]
			: []),
		"topics:",
		"  - rich-text",
		"  - editor",
		"  - canvist",
		"",
		"environment:",
		"  sdk: ^3.13.0",
		"",
		"dependencies:",
		`  canvist: ^${VERSION}`,
		"",
	].join("\n");
	const first = item.names[0];
	const readme = `# ${name}

${item.description}, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports ${
		item.names.map((extension) => `\`${extension}\``).join(", ")
	} from \`package:canvist\`, so you can import ${
		item.names.length === 1 ? "it" : "them"
	} on ${
		item.names.length === 1 ? "its" : "their"
	} own. canvist compiles every extension into its engine, so this package adds no code.

\`\`\`dart
import 'package:canvist/canvist.dart';
import 'package:${name}/${name}.dart';

final builder = EngineBuilder();
final ${first}Handle = builder.add(${first});
final engine = await builder.build();
\`\`\`

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
`;

	const overrides = [
		"# Build against the `canvist` package in this repository instead of the",
		"# pub.dev release. pub reads this file locally and never publishes it.",
		"dependency_overrides:",
		"  canvist:",
		"    path: ../../dart",
		"",
	].join("\n");

	return {
		"pubspec.yaml": pubspec,
		"pubspec_overrides.yaml": overrides,
		[`lib/${name}.dart`]: library,
		"README.md": readme,
		LICENSE,
	};
}

const check = process.argv.includes("--check");
const stale: string[] = [];

function write(directory: URL, files: Record<string, string>): void {
	for (const [path, contents] of Object.entries(files)) {
		const file = new URL(path, directory);

		if (existsSync(file) && readFileSync(file, "utf8") === contents) {
			continue;
		}

		if (check) {
			stale.push(file.pathname);
			continue;
		}

		mkdirSync(new URL(".", file), { recursive: true });
		writeFileSync(file, contents);
	}
}

for (const item of packages()) {
	write(new URL(`packages/extensions/${kebab(item.crate)}/`, root), npm(item));

	const directory = new URL(`platforms/dart_extensions/${item.crate}/`, root);
	const pubspec = new URL("pubspec.yaml", directory);
	const existing = existsSync(pubspec)
		? readFileSync(pubspec, "utf8")
		: undefined;
	write(directory, dart(item, existing));
}

if (stale.length > 0) {
	console.error(
		`out of date; run node tools/extension-packages.ts:\n${stale.join("\n")}`,
	);
	process.exitCode = 1;
}
