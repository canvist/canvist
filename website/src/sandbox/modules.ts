/**
 * What sandboxed code can import, in each of the two realms it runs in.
 *
 * - The **server** realm is a Web Worker: canvist's headless packages and
 *   the slice of Node and Deno that canvist servers use (`ws`, `node:http`,
 *   `node:events`, `Deno.serve`), over a network that only reaches the
 *   page's own sandbox.
 * - The **browser** realm is a frame with a DOM: the editor packages, with
 *   `WebSocket` and `fetch` to `localhost` routed to the server realm.
 *
 * The build reads this list to decide which docs samples get a Run button,
 * and the realms read it to load each module, so the two can't disagree.
 */

export type Realm = "server" | "browser";

export const SERVER_MODULES = [
	"@canvist/headless",
	"@canvist/headless/full",
	"@canvist/headless/ssr",
	"ws",
	"node:http",
	"node:events",
] as const;

export const BROWSER_MODULES = [
	"@canvist/web",
	"@canvist/web/collab",
	"@canvist/web/full",
	"@canvist/web/menus",
	"@canvist/headless",
	"@canvist/headless/full",
] as const;

export type ServerModule = (typeof SERVER_MODULES)[number];
export type BrowserModule = (typeof BROWSER_MODULES)[number];

/** Node resolves `http` and `node:http` to the same module. */
const NODE_ALIASES: Readonly<Record<string, ServerModule>> = {
	http: "node:http",
	events: "node:events",
};

/** The server module `specifier` names, if the server realm has it. */
export function serverModule(specifier: string): ServerModule | undefined {
	const name = NODE_ALIASES[specifier] ?? specifier;
	return SERVER_MODULES.find((candidate) => candidate === name);
}

/** The browser module `specifier` names, if the browser realm has it. */
export function browserModule(specifier: string): BrowserModule | undefined {
	return BROWSER_MODULES.find((candidate) => candidate === specifier);
}

const IMPORT = /\b(?:from|import)\s*\(?\s*["']([^"']+)["']/g;

/** The module specifiers TypeScript `source` imports, in order, once each. */
export function importsOf(source: string): string[] {
	return [
		...new Set([...source.matchAll(IMPORT)].map((match) => match[1] ?? "")),
	];
}

/**
 * The realm `source` runs in, or `undefined` when it imports something
 * neither realm has (a file system, React, a published Yjs provider).
 * Code that imports the editor needs a DOM; everything else runs on the
 * server realm, the way the docs' samples run under Deno.
 */
export function realmOf(source: string): Realm | undefined {
	const imports = importsOf(source);
	if (imports.some((specifier) => specifier.startsWith("@canvist/web"))) {
		return imports.every((specifier) => browserModule(specifier) !== undefined)
			? "browser"
			: undefined;
	}
	return imports.every((specifier) => serverModule(specifier) !== undefined)
		? "server"
		: undefined;
}
