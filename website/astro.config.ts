import { unified } from "@astrojs/markdown-remark";
import { defineConfig } from "astro/config";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { liveSamples } from "./src/lib/live-samples.ts";
import { rehypeDocsLinks } from "./src/lib/rehype-docs-links.ts";

const repoRoot = fileURLToPath(new URL("..", import.meta.url));

/**
 * The public origin, used for canonical URLs and Open Graph images. Railway
 * passes `RAILWAY_PUBLIC_DOMAIN` to the build; `SITE_URL` overrides it.
 */
function siteUrl(): string {
	const explicit = process.env["SITE_URL"];
	if (explicit) return explicit;
	const railway = process.env["RAILWAY_PUBLIC_DOMAIN"];
	if (railway) return `https://${railway}`;
	return "http://localhost:4321";
}

export default defineConfig({
	site: siteUrl(),
	output: "static",
	trailingSlash: "never",
	build: {
		format: "file",
		inlineStylesheets: "always",
	},
	prefetch: {
		prefetchAll: false,
		defaultStrategy: "hover",
	},
	markdown: {
		shikiConfig: {
			themes: { light: "github-light", dark: "github-dark" },
			defaultColor: false,
			wrap: false,
			transformers: [liveSamples],
		},
		processor: unified({ rehypePlugins: [rehypeDocsLinks] }),
	},
	vite: {
		define: {
			// Build-time file reads resolve from here; bundling moves import.meta.url.
			"import.meta.env.REPO_ROOT": JSON.stringify(repoRoot),
		},
		server: {
			// The site reads the docs and the brand assets from the rest of the
			// repository, and links the canvist packages from `packages/`.
			fs: { allow: [repoRoot] },
		},
		// The canvist packages find their WebAssembly next to their own
		// modules, and pre-bundling in development would move the modules
		// away from it.
		optimizeDeps: {
			exclude: [
				"@canvist/web",
				"@canvist/headless",
				"@canvist/collab",
				"@canvist/ssr",
			],
		},
		// The packages are linked from `packages/`, where each links its own
		// `@canvist/core`; the site resolves one copy, so `@canvist/headless`
		// and `@canvist/collab` share their classes.
		resolve: {
			dedupe: ["@canvist/core"],
		},
		build: {
			assetsInlineLimit: 0,
		},
		// The sandbox's server realm is a module worker that loads canvist's
		// packages on demand, which needs code splitting.
		worker: {
			format: "es",
		},
	},
});
