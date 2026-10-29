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

	test("the previewed document can be typed in, and the other tabs follow", async ({ page }) => {
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

		await sample.locator(".sandbox-preview canvas").click();
		await page.keyboard.press("ControlOrMeta+End");
		await page.keyboard.type(" now");
		await sample.getByRole("tab", { name: "Markdown" }).click();
		await expect(sample.locator(".sandbox-preview__source")).toHaveText(
			"**Ship it now**",
		);
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

	test("the Every extension document is an editor with every extension", async ({ page }) => {
		await openPlayground(page, "extensions");
		const preview = page.locator("[data-server-doc]");
		await preview.locator("canvas").click({ position: { x: 24, y: 12 } });
		await page.keyboard.press("Home");
		await page.keyboard.type("Typed: ");
		await preview.getByRole("tab", { name: "HTML" }).click();
		const html = preview.locator("iframe").contentFrame();
		await expect(html.locator("h1")).toHaveText(
			"Typed: Every extension, one document",
		);
		// Extensions beyond the StarterKit still lay out: the table is there.
		await expect(html.locator("table")).toBeVisible();
	});

	test("an editor switches to @canvist/web/full for extensions beyond the StarterKit", async ({ page }) => {
		await openPlayground(page, "editor");
		// Without a server there's nothing for another peer to share.
		await expect(page.getByRole("button", { name: "Add a peer" })).toBeHidden();

		await page.getByRole("checkbox", { name: "subscript", exact: true })
			.check();
		await expect.poll(() =>
			page.evaluate(() => window.canvistPlayground!.read("client.ts"))
		).toContain('from "@canvist/web/full"');
		const result = await page.evaluate(() => window.canvistPlayground!.run());
		expect(result.errors).toEqual([]);
		await expect(
			page.locator(".pg-peer iframe").contentFrame().getByRole("button", {
				name: "Subscript",
			}),
		).toBeVisible();
	});

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
		await expect(first.locator("#status")).toHaveText("Synced as Peer 1");
		await expect(second.locator("#status")).toHaveText("Synced as Peer 2");

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

		// The other peer sees where peer one is typing; peer one doesn't see itself.
		const cursor = second.locator(".cursor");
		await expect(cursor).toBeVisible();
		await expect(cursor).toHaveText("Peer 1");
		await expect(first.locator(".cursor:visible")).toHaveCount(0);

		// A peer that leaves takes its cursor with it.
		await page.getByRole("button", { name: "Remove peer 1" }).click();
		await expect(cursor).toHaveCount(0);
	});

	test("a new peer joins the room and scrolls into view", async ({ page }) => {
		await openPlayground(page, "collaboration");
		await page.getByRole("button", { name: "Add a peer" }).click();
		const third = page.locator(".pg-peer iframe").nth(2);
		await expect(third.contentFrame().locator("#status")).toHaveText(
			"Synced as Peer 3",
		);
		await expect(third).toBeInViewport();
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
