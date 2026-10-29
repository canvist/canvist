/**
 * `@canvist/web/full`, the build with every extension, and `caretRectAt`,
 * which hosts draw collaborators' carets with.
 *
 * @module
 */
import { expect, type Page, test } from "@playwright/test";
import type { allExtensions } from "../../src/full.ts";
import type { Editor, StarterKit } from "../../src/mod.ts";

declare global {
	var editor: Editor<StarterKit>;
	var fullEditor: Editor<typeof allExtensions>;
}

async function open(
	page: Page,
	path: "/editing.html" | "/full.html",
	markdown: string,
): Promise<void> {
	await page.goto(`${path}?${new URLSearchParams({ markdown })}`);
	await page.waitForFunction(() =>
		globalThis.editor !== undefined || globalThis.fullEditor !== undefined
	);
}

test("caretRectAt is where a caret at that position is drawn", async ({ page }) => {
	await open(page, "/editing.html", "Hello world\n\nSecond paragraph");
	const [at, drawn] = await page.evaluate(() => {
		const position = { path: [1], offset: 6 };
		const at = editor.caretRectAt(position)?.toJSON();
		editor.select(position);

		return [at, editor.caretRect()?.toJSON()];
	});

	expect(at).toBeDefined();
	expect(at).toEqual(drawn);
});

test("caretRectAt is null for a position the document doesn't have", async ({ page }) => {
	await open(page, "/editing.html", "Hello world");
	const missing = await page.evaluate(() => [
		editor.caretRectAt({ path: [4], offset: 0 }),
		editor.caretRectAt({ path: [0], offset: 99 }),
	]);

	expect(missing).toEqual([null, null]);
});

test("the full build lays out and edits extensions beyond the StarterKit", async ({ page }) => {
	await open(
		page,
		"/full.html",
		"| a | b |\n| - | - |\n| 1 | 2 |\n\nSome ==marked== text",
	);
	const result = await page.evaluate(() => {
		fullEditor.select({ path: [0, 1, 0, 0], offset: 1 });
		const added = fullEditor.commands.addRowAfter();

		return { added, html: fullEditor.export("html") };
	});

	expect(result.added).toBe(true);
	expect(result.html).toContain("<mark>marked</mark>");
	expect(result.html.match(/<tr>/g)).toHaveLength(3);
});
