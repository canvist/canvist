import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";

/**
 * The admin dashboard against a mocked canvist Cloud: the static build has
 * no server, so `/v1/admin` answers come from `page.route`.
 */

const DAY = 86_400_000;
const today = Math.floor(Date.now() / DAY) * DAY;

const summary = {
	visitors: 1_284,
	sessions: 1_502,
	pageviews: 4_210,
	bounce_rate: 0.41,
	average_session_seconds: 185,
};
const report = {
	from: today - 6 * DAY,
	to: today + DAY,
	summary,
	previous: { ...summary, visitors: 1_070 },
	days: Array.from({ length: 7 }, (_, index) => ({
		day: today - (6 - index) * DAY,
		visitors: [120, 180, 160, 240, 210, 190, 184][index] ?? 0,
		pageviews: 400,
	})),
	pages: [{ key: "/", count: 2_100, visitors: 900 }, {
		key: "/pricing",
		count: 640,
		visitors: 410,
	}],
	sources: [{ key: "direct", count: 800, visitors: 700 }, {
		key: "news.ycombinator.com",
		count: 300,
		visitors: 280,
	}],
	devices: [{ key: "desktop", count: 3_000, visitors: 900 }],
	browsers: [{ key: "Chrome", count: 2_400, visitors: 700 }],
	site_events: [{ key: "hero: start building", count: 120, visitors: 110 }],
	server_events: [{ key: "project created", count: 4, visitors: 4 }],
	truncated: false,
};
const projects = [
	{
		id: "prj_mine",
		name: "Side project",
		plan: "hobby",
		included_documents: 1_000,
		active_documents: 12,
		standing: "ok",
		subscription_status: null,
		stripe_customer: null,
	},
];

interface Mock {
	signedIn: boolean;
	coupons: unknown[];
	created: unknown[];
}

async function mockCloud(page: Page, mock: Mock): Promise<void> {
	await page.route("**/v1/admin/**", async (route) => {
		const request = route.request();
		const path = new URL(request.url()).pathname;
		const json = (status: number, body?: unknown) =>
			route.fulfill({
				status,
				contentType: "application/json",
				body: body === undefined ? "" : JSON.stringify(body),
			});

		if (path === "/v1/admin/session" && request.method() === "GET") {
			return mock.signedIn
				? json(200, { admin: "owner@canvist.test" })
				: json(401, {
					error: {
						code: "missing_credentials",
						message: "missing credentials",
					},
				});
		}
		if (path === "/v1/admin/session" && request.method() === "POST") {
			mock.signedIn = request.postDataJSON().token === "good-token";
			return mock.signedIn
				? json(200, { admin: "owner@canvist.test" })
				: json(401, {
					error: {
						code: "invalid_credentials",
						message: "invalid credentials",
					},
				});
		}
		if (path === "/v1/admin/sign-in") return json(202);
		if (path === "/v1/admin/analytics") return json(200, report);
		if (path === "/v1/admin/projects") {
			return json(200, { month: "2026-10", projects });
		}
		if (path === "/v1/admin/coupons" && request.method() === "POST") {
			const body = request.postDataJSON();
			mock.created.push(body);
			const coupon = {
				id: "co_1",
				name: body.name,
				percent_off: body.percent_off ?? null,
				amount_off: null,
				currency: null,
				duration: body.duration,
				duration_in_months: null,
				valid: true,
				times_redeemed: 0,
				created: 0,
				visibility: body.visibility,
				project: body.project ?? null,
				created_by: "owner@canvist.test",
				codes: [{
					id: "promo_1",
					code: "MINE100",
					active: true,
					customer: "cus_1",
					times_redeemed: 0,
					max_redemptions: null,
					expires_at: null,
				}],
			};
			mock.coupons.push(coupon);
			return json(201, { coupon, applied_to_subscription: false });
		}
		if (path === "/v1/admin/coupons") {
			return json(200, { coupons: mock.coupons });
		}
		return json(404, { error: { code: "not_found", message: "not found" } });
	});
}

test.describe("admin", () => {
	test("signed out, it asks for an email and never says who is an admin", async ({ page }) => {
		await mockCloud(page, { signedIn: false, coupons: [], created: [] });
		await page.goto("/admin");
		await expect(page).toHaveTitle(/Admin · canvist/);
		await expect(page.locator('meta[name="robots"]')).toHaveAttribute(
			"content",
			"noindex, nofollow",
		);

		const signIn = page.waitForRequest("**/v1/admin/sign-in");
		await page.getByLabel("Email").fill("owner@canvist.test");
		await page.getByRole("button", { name: "Email me a link" }).click();
		expect((await signIn).postDataJSON()).toEqual({
			email: "owner@canvist.test",
		});
		await expect(
			page.getByText("If that address can sign in, a link is on its way."),
		).toBeVisible();
	});

	test("a sign-in link opens the dashboard and leaves the address bar", async ({ page }) => {
		await mockCloud(page, { signedIn: false, coupons: [], created: [] });
		await page.goto("/admin#token=good-token");

		await expect(page.getByText("Signed in as owner@canvist.test"))
			.toBeVisible();
		expect(new URL(page.url()).hash).toBe("");
		await expect(page.getByRole("tab", { name: "Analytics" })).toHaveAttribute(
			"aria-selected",
			"true",
		);
		await expect(page.locator(".tile__value").first()).toHaveText("1,284");
		await expect(page.getByText("+20% on the 30 days before")).toBeVisible();
		await expect(page.getByRole("img", { name: /Visitors per day/ }))
			.toBeVisible();
		await expect(page.getByRole("rowheader", { name: "news.ycombinator.com" }))
			.toBeVisible();

		const results = await new AxeBuilder({ page }).withTags([
			"wcag2a",
			"wcag2aa",
			"wcag21aa",
			"wcag22aa",
		]).analyze();
		const serious = results.violations.filter((violation) =>
			violation.impact === "serious" || violation.impact === "critical"
		);
		expect(serious.map((violation) => violation.id)).toEqual([]);
	});

	test("an expired link asks for a new one", async ({ page }) => {
		await mockCloud(page, { signedIn: false, coupons: [], created: [] });
		await page.goto("/admin#token=stale");
		await expect(page.getByText("That link has expired. Ask for a new one."))
			.toBeVisible();
	});

	test("a project gets a private coupon from a preset", async ({ page }) => {
		const mock: Mock = { signedIn: true, coupons: [], created: [] };
		await mockCloud(page, mock);
		await page.goto("/admin");

		await page.getByRole("tab", { name: "Projects" }).click();
		await page.getByRole("row", { name: /Side project/ }).getByRole("button", {
			name: "Give a coupon",
		}).click();
		await expect(page.getByRole("tab", { name: "Coupons" })).toHaveAttribute(
			"aria-selected",
			"true",
		);
		await page.getByRole("button", { name: "Free forever" }).click();
		await expect(page.getByRole("combobox", { name: "Project" })).toHaveValue(
			"prj_mine",
		);
		await page.getByRole("button", { name: "Create coupon" }).click();

		await expect(
			page.getByText("Made MINE100. Only that project can redeem it."),
		).toBeVisible();
		expect(mock.created).toEqual([
			{
				name: "Free forever",
				percent_off: 100,
				duration: "forever",
				visibility: "private",
				project: "prj_mine",
			},
		]);
		await expect(page.locator(".coupon")).toContainText("100% off forever");
		await expect(page.locator(".coupon")).toContainText(
			"Private to Side project · MINE100",
		);
	});

	test("deleting a coupon asks first", async ({ page }) => {
		const mock: Mock = { signedIn: true, coupons: [], created: [] };
		await mockCloud(page, mock);
		await page.goto("/admin");
		await page.getByRole("tab", { name: "Coupons" }).click();
		await page.getByRole("button", { name: "Launch code" }).click();
		await page.getByRole("button", { name: "Create coupon" }).click();
		await expect(page.locator(".coupon")).toHaveCount(1);

		await page.getByRole("button", { name: "Delete", exact: true }).click();
		await expect(page.getByText("Delete this coupon?")).toBeVisible();
		await page.getByRole("button", { name: "Keep" }).click();
		await expect(page.getByRole("button", { name: "Delete", exact: true }))
			.toBeVisible();
	});
});
