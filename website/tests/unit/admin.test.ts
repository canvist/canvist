import { assertEquals } from "@std/assert";
import {
	columnPath,
	columns,
	type Coupon,
	describeDiscount,
	endOfDay,
	formatChange,
	formatCount,
	formatMoney,
	formatSeconds,
	formatShare,
	niceCeiling,
	PRESETS,
} from "../../src/lib/admin.ts";

const DAY = 86_400_000;

function coupon(overrides: Partial<Coupon>): Coupon {
	return {
		id: "co_1",
		name: "Test",
		percent_off: null,
		amount_off: null,
		currency: null,
		duration: "once",
		duration_in_months: null,
		valid: true,
		times_redeemed: 0,
		created: 0,
		visibility: "public",
		project: null,
		created_by: null,
		codes: [],
		...overrides,
	};
}

Deno.test("numbers compact only when they are large", () => {
	assertEquals(formatCount(0), "0");
	assertEquals(formatCount(9_999), "9,999");
	assertEquals(formatCount(12_900), "12.9K");
	assertEquals(formatShare(0.4321), "43%");
});

Deno.test("session lengths read as seconds, minutes, or hours", () => {
	assertEquals(formatSeconds(45), "45s");
	assertEquals(formatSeconds(185), "3m 05s");
	assertEquals(formatSeconds(3_720), "1h 02m");
	assertEquals(formatSeconds(-3), "0s");
});

Deno.test("changes compare with the period before", () => {
	assertEquals(formatChange(120, 100), "+20%");
	assertEquals(formatChange(97, 100), "−3%");
	assertEquals(formatChange(5, 0), "new");
	assertEquals(formatChange(0, 0), "no change");
	assertEquals(formatChange(1001, 1000), "no change");
});

Deno.test("discounts describe their amount and how long they last", () => {
	assertEquals(
		describeDiscount(coupon({ percent_off: 100, duration: "forever" })),
		"100% off forever",
	);
	assertEquals(
		describeDiscount(
			coupon({
				percent_off: 50,
				duration: "repeating",
				duration_in_months: 12,
			}),
		),
		"50% off for 12 months",
	);
	assertEquals(
		describeDiscount(
			coupon({
				percent_off: 100,
				duration: "repeating",
				duration_in_months: 1,
			}),
		),
		"100% off for 1 month",
	);
	assertEquals(
		describeDiscount(coupon({ amount_off: 1000, currency: "usd" })),
		"$10 off once",
	);
	assertEquals(formatMoney(450, "eur"), "€4.50");
});

Deno.test("expiry dates end with their UTC day", () => {
	assertEquals(
		endOfDay("2026-12-31"),
		Date.UTC(2026, 11, 31, 23, 59, 59) / 1000,
	);
	assertEquals(endOfDay(""), undefined);
	assertEquals(endOfDay("31/12/2026"), undefined);
});

Deno.test("chart ceilings are round", () => {
	assertEquals(niceCeiling(0), 1);
	assertEquals(niceCeiling(7), 10);
	assertEquals(niceCeiling(18), 20);
	assertEquals(niceCeiling(230), 250);
	assertEquals(niceCeiling(4_100), 5_000);
});

Deno.test("columns stay thin, grow from the baseline, and round only their tops", () => {
	const days = [0, 1, 2].map((index) => ({
		day: index * DAY,
		visitors: index * 5,
		pageviews: 0,
	}));
	const placed = columns(days, 300, 100, 10);

	assertEquals(placed.map((column) => column.width), [24, 24, 24]);
	assertEquals(placed.map((column) => column.y + column.height), [
		100,
		100,
		100,
	]);
	assertEquals(placed[2]?.height, 100);
	assertEquals(columnPath(placed[0]!), "", "an empty day draws nothing");
	assertEquals(columnPath(placed[1]!).startsWith(`M${placed[1]!.x},100`), true);
});

Deno.test("presets cover the pricing page's promises", () => {
	const names = PRESETS.map((preset) => preset.coupon.name);
	assertEquals(names.includes("Open source: Builder free"), true);
	assertEquals(names.includes("Small team: 50% off a year"), true);
	for (const preset of PRESETS) {
		assertEquals(
			preset.coupon.name.length <= 40,
			true,
			`${preset.label} fits Stripe's 40 characters`,
		);
	}
});
