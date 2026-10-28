import { expect, type Page, test } from "@playwright/test";

/**
 * The sandbox in the built site: docs samples that run on the page, and the
 * playground's examples, extension switches, and collaboration between
 * browser peers through a server running in the tab.
 */

const playgroundState = (page: Page) => page.locator("[data-playground]");

async function openPlayground(page: Page, example: string): Promise<void> {
	await page.goto(`/playground?example=${example}`);
	await expect(playgroundState(page)).toHaveAttribute("data-state", "ok", {
		timeout: 30_000,
	});
}

test.describe("docs samples", () => {
	test("a server sample runs, prints what its comments say, and previews its document", async ({ page }) => {
		await page.goto("/docs/extensions");
		const sample = page.locator(".docs-code").filter({
			has: page.locator("pre[data-live]"),
		}).first();
		await sample.getByRole("button", {
			name: "Run this sample, and edit it live",
		}).click();

		await expect(sample).toHaveAttribute("data-state", "ok", {
			timeout: 30_000,
		});
		await expect(sample.getByRole("status")).toContainText(
			"printed what the comments say",
		);
		await expect(sample.locator(".sandbox-console")).toHaveText(
			"<p><strong>Ship it</strong></p>",
		);
		await expect(sample.getByRole("tab", { name: "Editor" })).toHaveAttribute(
			"aria-selected",
			"true",
		);
		await expect(sample.locator(".sandbox-preview canvas")).toBeVisible();
	});

	test("editing a sample runs it again, and an error points at its line", async ({ page }) => {
		await page.goto("/docs/extensions");
		const sample = page.locator(".docs-code").filter({
			has: page.locator("pre[data-live]"),
		}).first();
		await sample.getByRole("button", {
			name: "Run this sample, and edit it live",
		}).click();
		await expect(sample).toHaveAttribute("data-state", "ok", {
			timeout: 30_000,
		});

		const editor = sample.getByRole("textbox", { name: /Sample code/ });
		await editor.click();
		await page.keyboard.press("ControlOrMeta+End");
		await page.keyboard.type('\nthrow new Error("on purpose");');
		await expect(sample).toHaveAttribute("data-state", "error");
		await expect(sample.getByRole("status")).toHaveText("Failed on line 8");
		await expect(sample.locator(".cm-error-line")).toHaveCount(1);

		await sample.getByRole("button", { name: "Reset" }).click();
		await expect(sample).toHaveAttribute("data-state", "ok");
	});

	test("a browser sample runs against the markup before it", async ({ page }) => {
		await page.goto("/docs/guides/toolbar");
		const sample = page.locator(".docs-code").filter({
			has: page.locator('pre[data-live="browser"]'),
		});
		await sample.getByRole("button", {
			name: "Run this sample, and edit it live",
		}).click();

		await expect(sample).toHaveAttribute("data-state", "ok", {
			timeout: 30_000,
		});
		const frame = sample.locator("iframe").contentFrame();
		await expect(frame.getByRole("toolbar").getByRole("button")).toHaveText([
			"B",
			"I",
		]);
		await expect(frame.locator("#editor canvas")).toBeVisible();
	});
});

test.describe("playground", () => {
	for (const example of ["editor", "extensions", "collaboration", "node"]) {
		test(`the ${example} example runs without errors`, async ({ page }) => {
			const errors: string[] = [];
			page.on("pageerror", (error) => errors.push(error.message));
			await openPlayground(page, example);
			const result = await page.evaluate(() => window.canvistPlayground!.run());
			expect(result.errors).toEqual([]);
			expect(errors).toEqual([]);
		});
	}

	test("switching an extension rewrites the code and changes the document", async ({ page }) => {
		await openPlayground(page, "extensions");
		const before = await page.evaluate(() =>
			window.canvistPlayground!.documents()[0]?.html ?? ""
		);
		expect(before).toContain("<table>");

		await page.getByRole("checkbox", { name: "table", exact: true }).click();
		await expect.poll(() =>
			page.evaluate(() => window.canvistPlayground!.read("server.ts"))
		).toContain("...starterKit");
		const result = await page.evaluate(() => window.canvistPlayground!.run());
		expect(result.ok).toBe(true);
		expect(result.documents[0]?.html).not.toContain("<table>");
		expect(
			await page.evaluate(() => window.canvistPlayground!.extensions().enabled),
		).not.toContain("table");
	});

	test("browser peers collaborate through the server running in the tab", async ({ page }) => {
		await openPlayground(page, "collaboration");
		const frames = page.locator(".pg-peer iframe");
		await expect(frames).toHaveCount(2);
		const [first, second] = [
			frames.nth(0).contentFrame(),
			frames.nth(1).contentFrame(),
		];
		await expect(first.locator("#status")).toHaveText("Synced");
		await expect(second.locator("#status")).toHaveText("Synced");

		await first.locator("#editor canvas").first().click();
		await page.keyboard.press("ControlOrMeta+End");
		await page.keyboard.type(" Hello from peer one");

		await expect.poll(
			() =>
				page.evaluate(() =>
					window.canvistPlayground!.documents()[0]?.markdown ?? ""
				),
			{ timeout: 10_000 },
		)
			.toContain("Hello from peer one");
		await expect(page.locator("[data-ports]")).toHaveText("localhost:8787");
	});

	test("the page can call the server realm over fetch", async ({ page }) => {
		await openPlayground(page, "node");
		const html = await page.evaluate(async () => {
			const response = await window.canvistPlayground!.fetch(
				"http://localhost:3000/convert?to=html",
				{ method: "POST", body: "# Hi" },
			);
			return response.text();
		});
		expect(html).toBe("<h1>Hi</h1>");
	});
});
