import type { ShikiConfig } from "@astrojs/markdown-remark";
import { type Realm, realmOf } from "../sandbox/modules.ts";

/**
 * Marks the docs samples a reader can edit and run on the page, at build
 * time, so the page's script only enhances what can really run.
 *
 * A `ts` sample is live when everything it imports is in the sandbox, and
 * it either runs (no `no_run`, so `docs/check-snippets.ts` runs it under
 * Deno too) or says `live`: a browser sample known to run against the
 * markup in the `html` sample before it. Live samples carry the realm they
 * run in as `data-live` on their `<pre>`.
 */

type ShikiTransformer = NonNullable<ShikiConfig["transformers"]>[number];

/** The realm a fence runs in on the page, or `undefined` when it only shows. */
export function liveRealm(
	lang: string,
	meta: string,
	code: string,
): Realm | undefined {
	if (lang !== "ts") return undefined;
	const flags = meta.split(/\s+/).filter(Boolean);
	if (flags.includes("no_run") && !flags.includes("live")) return undefined;
	return realmOf(code);
}

export const liveSamples: ShikiTransformer = {
	name: "canvist:live-samples",
	pre(node) {
		const realm = liveRealm(
			this.options.lang,
			this.options.meta?.__raw ?? "",
			this.source,
		);
		if (realm !== undefined) node.properties["data-live"] = realm;
	},
};
