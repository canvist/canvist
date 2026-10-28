/**
 * Execute the relink against a synthetic workspace: ranges that name
 * workspace packages go back to `file:` links, everything else survives,
 * and a second run changes nothing. Run from the repository root:
 * deno test --allow-read --allow-write tools/release/relink_workspace_deps_test.ts
 */
import { assert, assertEquals } from "jsr:@std/assert@^1";
import { relativePath, relinkWorkspaceDeps } from "./relink_workspace_deps.ts";

async function writeJson(path: string, value: unknown): Promise<void> {
	await Deno.mkdir(path.slice(0, path.lastIndexOf("/")), { recursive: true });
	await Deno.writeTextFile(path, `${JSON.stringify(value, null, "\t")}\n`);
}

async function readJson(path: string): Promise<Record<string, unknown>> {
	return JSON.parse(await Deno.readTextFile(path)) as Record<string, unknown>;
}

Deno.test("relinks workspace ranges to file links and preserves everything else", async () => {
	const root = await Deno.makeTempDir();
	try {
		await writeJson(`${root}/packages/alpha/package.json`, {
			name: "@canvist/alpha",
			version: "0.2.0",
			dependencies: { "yjs": "13.6.33" },
		});
		await writeJson(`${root}/packages/beta/package.json`, {
			name: "@canvist/beta",
			version: "0.2.0",
			dependencies: {
				"@canvist/alpha": "^0.2.0",
				"@canvist/alpha/scene": "^0.2.0",
			},
		});
		await writeJson(`${root}/website/package.json`, {
			name: "@canvist/website",
			private: true,
			dependencies: {
				"@canvist/alpha": "^0.2.0",
				"@canvist/beta": "file:../packages/beta",
				"@canvist/stranger": "^9.0.0",
			},
			devDependencies: { "@canvist/alpha": "workspace:*" },
		});

		const rewritten = await relinkWorkspaceDeps(root);

		assertEquals(rewritten, [
			"packages/beta/package.json -> dependencies.@canvist/alpha",
			"website/package.json -> dependencies.@canvist/alpha",
		]);

		const beta = await readJson(`${root}/packages/beta/package.json`);
		assertEquals(
			(beta.dependencies as Record<string, string>)["@canvist/alpha"],
			"file:../alpha",
		);
		const website = await readJson(`${root}/website/package.json`);
		assertEquals(
			(website.dependencies as Record<string, string>)["@canvist/alpha"],
			"file:../packages/alpha",
		);
		assertEquals(
			(website.dependencies as Record<string, string>)["@canvist/beta"],
			"file:../packages/beta",
		);
		assertEquals(
			(website.dependencies as Record<string, string>)["@canvist/stranger"],
			"^9.0.0",
		);
		assertEquals(
			(website.devDependencies as Record<string, string>)["@canvist/alpha"],
			"workspace:*",
		);

		// The rewritten manifests keep their formatting: tabs and a final newline.
		const source = await Deno.readTextFile(`${root}/website/package.json`);
		assert(source.startsWith('{\n\t"name"'), source);
		assert(source.endsWith("}\n"), source);
	} finally {
		await Deno.remove(root, { recursive: true });
	}
});

Deno.test("a second run is a no-op", async () => {
	const root = await Deno.makeTempDir();
	try {
		await writeJson(`${root}/packages/alpha/package.json`, {
			name: "@canvist/alpha",
		});
		await writeJson(`${root}/website/package.json`, {
			name: "@canvist/website",
			dependencies: { "@canvist/alpha": "^0.2.0" },
		});

		assert((await relinkWorkspaceDeps(root)).length > 0);
		assertEquals(await relinkWorkspaceDeps(root), []);
	} finally {
		await Deno.remove(root, { recursive: true });
	}
});

Deno.test("relative paths climb from the manifest to the package", () => {
	assertEquals(
		relativePath("website", "packages/canvist-web"),
		"../packages/canvist-web",
	);
	assertEquals(relativePath("packages/beta", "packages/alpha"), "../alpha");
	assertEquals(relativePath("website", "website"), ".");
});
