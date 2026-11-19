/**
 * Editors destroyed while their work is still in flight, as when a
 * component unmounts in the editor's first moments: fonts loading, plugins
 * setting up, and server-rendered HTML waiting to hydrate. None of it may
 * reach the released WebAssembly half or leave anything in the page.
 *
 * @module
 */
import { expect, type Page, test } from "@playwright/test";
import type { fonts } from "../../src/fonts.ts";
import type { createEditor } from "../../src/mod.ts";

declare global {
	// What `tests/pages/lifecycle.ts` puts on `globalThis`.
	var canvist: {
		createEditor: typeof createEditor;
		fonts: typeof fonts;
	};
}

/** Open the lifecycle page, collecting uncaught errors and unhandled rejections. */
async function open(page: Page): Promise<string[]> {
	const errors: string[] = [];
	page.on("pageerror", (error) => errors.push(error.message));
	await page.goto("/lifecycle.html");
	await page.waitForFunction(() => globalThis.canvist !== undefined);

	return errors;
}

test("destroying an editor twice is the same as once", async ({ page }) => {
	const errors = await open(page);
	const elements = await page.evaluate(async () => {
		const host = document.createElement("div");
		document.body.append(host);
		const editor = await canvist.createEditor(host);
		editor.destroy();
		editor.destroy();

		return host.childElementCount;
	});

	expect(elements).toBe(0);
	expect(errors).toEqual([]);
});

test("an editor destroyed before it hydrates leaves the server's HTML in place", async ({ page }) => {
	const errors = await open(page);
	const left = await page.evaluate(async () => {
		const host = document.createElement("div");
		host.style.width = "320px";
		host.innerHTML = `<div class="cv-doc"><p>Rendered on the server</p></div>`;
		document.body.append(host);
		const editor = await canvist.createEditor(host);
		editor.destroy();
		await editor.ready();

		return [...host.children].map((child) => child.className);
	});

	expect(left).toEqual(["cv-doc"]);
	expect(errors).toEqual([]);
});

test("fonts that finish loading after the editor is destroyed change nothing", async ({ page }) => {
	const errors = await open(page);
	const added = await page.evaluate(async () => {
		const host = document.createElement("div");
		host.style.width = "320px";
		document.body.append(host);
		const face = {
			family: "Lifecycle Plugin",
			source: "/inter-variable.ttf",
			display: "block",
		} as const;
		const editor = await canvist.createEditor(host, {
			content: { format: "markdown", input: "Hello" },
			// Declared by the page, so the editor's own font watcher loads it.
			theme: { bodyFamily: `"Lifecycle Watched", serif` },
			plugins: [canvist.fonts({ faces: [face] })],
		});
		const loading = editor.plugins.fonts.load({
			...face,
			family: "Lifecycle Late",
		});
		editor.destroy();
		await loading;
		await document.fonts.ready;
		// Past the font watcher's callbacks and any frame they asked for.
		await new Promise((resolve) => requestAnimationFrame(resolve));
		await new Promise((resolve) => setTimeout(resolve, 100));

		return [...document.fonts]
			.map((font) => font.family.replace(/"/g, ""))
			.filter((family) => family !== "Lifecycle Watched");
	});

	expect(added).toEqual([]);
	expect(errors).toEqual([]);
});

test("an editor whose content fails to load leaves the element as it was", async ({ page }) => {
	const errors = await open(page);
	const result = await page.evaluate(async () => {
		const host = document.createElement("div");
		document.body.append(host);
		// A block no registered extension defines.
		const content = { type: "doc", content: [{ type: "unknown" }] } as never;
		const code = await canvist.createEditor(host, { content }).then(
			() => "created",
			(error: { readonly code?: string }) => error.code,
		);

		return { code, left: host.childElementCount };
	});

	expect(result).toEqual({ code: "schema", left: 0 });
	expect(errors).toEqual([]);
});
