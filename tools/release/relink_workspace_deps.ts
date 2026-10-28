/**
 * Repoints every internal `@canvist/*` dependency to its workspace package.
 *
 * Release preparation rewrites internal dependencies to the planned version
 * range (`^0.2.0`), which is what published packages must declare. The
 * website and the browser demo are never published, so for them the range
 * names a package that will not exist on npm until after the release
 * publishes, and every `deno install --frozen` on the release pull request
 * fails on it. This script rewrites those ranges back to `file:` links for
 * the local build, the same shape the manifests carry between releases.
 * Run it before any deno resolution on a release branch; on other branches
 * it changes nothing:
 *
 *   deno run --allow-read --allow-write tools/release/relink_workspace_deps.ts
 */

/** Manifests that may depend on workspace packages by npm name. */
const MANIFEST_DIRS = ["packages/*", "services/*", "website"];

const DEPENDENCY_FIELDS = [
	"dependencies",
	"devDependencies",
	"peerDependencies",
	"optionalDependencies",
] as const;

/** Dependency values that already resolve locally and must not be touched. */
const LOCAL_SCHEMES = /^(file|workspace|link):/;

interface Manifest {
	name?: unknown;
	dependencies?: Record<string, string>;
	devDependencies?: Record<string, string>;
	peerDependencies?: Record<string, string>;
	optionalDependencies?: Record<string, string>;
}

/** Every `dir/package.json` under `root` the repo layout can hold. */
export async function workspaceManifests(root: string): Promise<string[]> {
	const manifests: string[] = [];
	for (const entry of MANIFEST_DIRS) {
		if (entry.includes("*")) {
			const [parent] = entry.split("/");
			for (const dir of listDirs(`${root}/${parent}`)) {
				const path = `${parent}/${dir}/package.json`;
				if (await exists(`${root}/${path}`)) manifests.push(path);
			}
		} else {
			const path = `${entry}/package.json`;
			if (await exists(`${root}/${path}`)) manifests.push(path);
		}
	}
	return manifests;
}

function listDirs(path: string): string[] {
	try {
		return [...Deno.readDirSync(path)].filter((entry) => entry.isDirectory).map(
			(entry) => entry.name,
		);
	} catch (error) {
		if (error instanceof Deno.errors.NotFound) return [];
		throw error;
	}
}

async function exists(path: string): Promise<boolean> {
	try {
		await Deno.stat(path);
		return true;
	} catch {
		return false;
	}
}

/**
 * Rewrite ranges that name workspace packages back to `file:` links in every
 * manifest under `root`. Returns the rewrites as `manifest -> field.name`.
 * Values that already resolve locally and dependencies outside the workspace
 * are left alone.
 */
export async function relinkWorkspaceDeps(root: string): Promise<string[]> {
	const manifests = await workspaceManifests(root);
	const packages = new Map<string, string>();
	for (const manifestPath of manifests) {
		const manifest = JSON.parse(
			await Deno.readTextFile(`${root}/${manifestPath}`),
		) as Manifest;
		if (typeof manifest.name === "string") {
			packages.set(manifest.name, dirname(manifestPath));
		}
	}

	const rewritten: string[] = [];
	for (const manifestPath of manifests) {
		const source = await Deno.readTextFile(`${root}/${manifestPath}`);
		const manifest = JSON.parse(source) as Manifest;
		let changed = false;
		for (const field of DEPENDENCY_FIELDS) {
			const dependencies = manifest[field];
			if (dependencies === undefined) continue;
			for (const [name, value] of Object.entries(dependencies)) {
				const target = packages.get(name);
				if (target === undefined || LOCAL_SCHEMES.test(value)) continue;
				dependencies[name] = `file:${
					relativePath(dirname(manifestPath), target)
				}`;
				rewritten.push(`${manifestPath} -> ${field}.${name}`);
				changed = true;
			}
		}
		if (changed) {
			await Deno.writeTextFile(
				`${root}/${manifestPath}`,
				`${JSON.stringify(manifest, null, "\t")}\n`,
			);
		}
	}
	return rewritten;
}

function dirname(path: string): string {
	return path.slice(0, path.lastIndexOf("/"));
}

/** The relative path from one workspace directory to another. */
export function relativePath(from: string, to: string): string {
	if (from === to) return ".";
	const fromParts = from.split("/");
	const toParts = to.split("/");
	let shared = 0;
	while (
		shared < fromParts.length && shared < toParts.length &&
		fromParts[shared] === toParts[shared]
	) {
		shared += 1;
	}
	return `${"../".repeat(fromParts.length - shared)}${
		toParts.slice(shared).join("/")
	}`;
}

if (import.meta.main) {
	const root = new URL("../../", import.meta.url).pathname;
	const rewrites = await relinkWorkspaceDeps(root);
	if (rewrites.length === 0) {
		console.log("internal dependencies already resolve inside the workspace");
	} else {
		for (const rewrite of rewrites) console.log(`relinked ${rewrite}`);
	}
}
