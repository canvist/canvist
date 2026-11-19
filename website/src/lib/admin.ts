/**
 * The admin dashboard's data: the shapes canvist Cloud's `/v1/admin` routes
 * answer with (services/cloud/src/admin), and the pure formatting the page
 * draws them with. `tests/unit/admin.test.ts` covers this file.
 */

export interface Summary {
	visitors: number;
	sessions: number;
	pageviews: number;
	bounce_rate: number;
	average_session_seconds: number;
}

export interface Day {
	/** The UTC day's first millisecond. */
	day: number;
	visitors: number;
	pageviews: number;
}

export interface Count {
	key: string;
	count: number;
	visitors: number;
}

export interface Report {
	from: number;
	to: number;
	summary: Summary;
	previous: Summary;
	days: Day[];
	pages: Count[];
	sources: Count[];
	devices: Count[];
	browsers: Count[];
	site_events: Count[];
	server_events: Count[];
	truncated: boolean;
}

export type Plan = "hobby" | "builder" | "scale" | "enterprise";
export type Standing = "ok" | "approaching" | "over" | "capped";

export interface Project {
	id: string;
	name: string;
	plan: Plan;
	included_documents: number | null;
	active_documents: number;
	standing: Standing;
	subscription_status: string | null;
	stripe_customer: string | null;
}

export interface Code {
	id: string;
	code: string;
	active: boolean;
	customer: string | null;
	times_redeemed: number;
	max_redemptions: number | null;
	expires_at: number | null;
}

export type Duration = "once" | "repeating" | "forever";
export type Visibility = "public" | "private";

export interface Coupon {
	id: string;
	name: string | null;
	percent_off: number | null;
	amount_off: number | null;
	currency: string | null;
	duration: Duration | null;
	duration_in_months: number | null;
	valid: boolean;
	times_redeemed: number;
	created: number;
	visibility: Visibility;
	project: string | null;
	created_by: string | null;
	codes: Code[];
}

/** What `POST /v1/admin/coupons` takes. */
export type NewCoupon =
	& {
		name: string;
		duration: Duration;
		duration_in_months?: number;
		max_redemptions?: number;
		expires_at?: number;
	}
	& ({ percent_off: number } | { amount_off: number; currency: string })
	& ({ visibility: "public"; code?: string } | {
		visibility: "private";
		project: string;
	});

/** `Omit` applied to each member of a union, keeping it a union. */
type OmitEach<T, K extends PropertyKey> = T extends unknown ? Omit<T, K>
	: never;

/** A starting point for a coupon: the offers canvist makes most. */
export interface Preset {
	label: string;
	coupon: OmitEach<NewCoupon, "project" | "code">;
}

/**
 * The offers the pricing page promises, and the ones the owner asked for.
 * Private ones still need a project picked.
 */
export const PRESETS: Preset[] = [
	{
		label: "Free forever",
		coupon: {
			name: "Free forever",
			percent_off: 100,
			duration: "forever",
			visibility: "private",
		},
	},
	{
		label: "Open source",
		coupon: {
			name: "Open source: Builder free",
			percent_off: 100,
			duration: "forever",
			visibility: "private",
		},
	},
	{
		label: "Free trial",
		coupon: {
			name: "Free for 3 months",
			percent_off: 100,
			duration: "repeating",
			duration_in_months: 3,
			visibility: "private",
		},
	},
	{
		label: "Small team",
		coupon: {
			name: "Small team: 50% off a year",
			percent_off: 50,
			duration: "repeating",
			duration_in_months: 12,
			visibility: "private",
		},
	},
	{
		label: "Launch code",
		coupon: {
			name: "Launch: 20% off",
			percent_off: 20,
			duration: "once",
			visibility: "public",
		},
	},
];

const whole = new Intl.NumberFormat("en-US");
const compact = new Intl.NumberFormat("en-US", {
	notation: "compact",
	maximumFractionDigits: 1,
});

/** 1,284 below ten thousand, 12.9K above. */
export function formatCount(value: number): string {
	return value < 10_000 ? whole.format(value) : compact.format(value);
}

/** `0.4321` as `43%`. */
export function formatShare(value: number): string {
	return `${Math.round(value * 100)}%`;
}

/** Seconds as `45s`, `3m 05s`, or `1h 02m`. */
export function formatSeconds(seconds: number): string {
	const total = Math.max(0, Math.round(seconds));
	if (total < 60) return `${total}s`;
	const minutes = Math.floor(total / 60);
	if (minutes < 60) {
		return `${minutes}m ${String(total % 60).padStart(2, "0")}s`;
	}
	return `${Math.floor(minutes / 60)}h ${
		String(minutes % 60).padStart(2, "0")
	}m`;
}

/** The change from `previous` to `current`: `+12%`, `−3%`, `new`, or `no change`. */
export function formatChange(current: number, previous: number): string {
	if (current === previous) return "no change";
	if (previous === 0) return "new";
	const change = Math.round(((current - previous) / previous) * 100);
	if (change === 0) return "no change";
	return change > 0 ? `+${change}%` : `−${Math.abs(change)}%`;
}

/** A UTC day as `Jan 10`. */
export function formatDay(millis: number): string {
	return new Date(millis).toLocaleDateString("en-US", {
		month: "short",
		day: "numeric",
		timeZone: "UTC",
	});
}

/** An amount in a currency's smallest unit, as `$29` or `€4.50`. */
export function formatMoney(amount: number, currency: string): string {
	const format = new Intl.NumberFormat("en-US", {
		style: "currency",
		currency: currency.toUpperCase(),
	});
	const value = format.format(amount / 100);
	return value.endsWith(".00") ? value.slice(0, -3) : value;
}

/** `100% off forever`, `$10 off once`, `50% off for 12 months`. */
export function describeDiscount(coupon: Coupon): string {
	const off = coupon.percent_off !== null
		? `${coupon.percent_off}% off`
		: `${formatMoney(coupon.amount_off ?? 0, coupon.currency ?? "usd")} off`;
	switch (coupon.duration) {
		case "forever":
			return `${off} forever`;
		case "repeating": {
			const months = coupon.duration_in_months ?? 0;
			return `${off} for ${months} month${months === 1 ? "" : "s"}`;
		}
		default:
			return `${off} once`;
	}
}

/** The end of a `YYYY-MM-DD` day in UTC, in Unix seconds, or `undefined`. */
export function endOfDay(date: string): number | undefined {
	const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date);
	if (!match) return undefined;
	const [, year, month, day] = match.map(Number);
	if (year === undefined || month === undefined || day === undefined) {
		return undefined;
	}
	return Date.UTC(year, month - 1, day, 23, 59, 59) / 1000;
}

/** The smallest round number at or above `value`, for a chart's top tick. */
export function niceCeiling(value: number): number {
	if (value <= 0) return 1;
	const magnitude = 10 ** Math.floor(Math.log10(value));
	for (const step of [1, 2, 2.5, 5, 10]) {
		const ceiling = step * magnitude;
		if (ceiling >= value) return ceiling;
	}
	return 10 * magnitude;
}

/** One column of the daily chart, in the chart's own units. */
export interface Column {
	x: number;
	y: number;
	width: number;
	height: number;
	day: Day;
}

/**
 * Columns for `days` in a `width` × `height` plot whose top is `ceiling`:
 * each takes its band, at most 24 units wide, growing from the baseline.
 */
export function columns(
	days: Day[],
	width: number,
	height: number,
	ceiling: number,
): Column[] {
	const band = width / Math.max(days.length, 1);
	const columnWidth = Math.min(24, Math.max(2, band - 2));
	return days.map((day, index) => {
		const columnHeight = (day.visitors / ceiling) * height;
		return {
			x: index * band + (band - columnWidth) / 2,
			y: height - columnHeight,
			width: columnWidth,
			height: columnHeight,
			day,
		};
	});
}

/** An SVG path for a column: square at the baseline, 4-unit rounded top. */
export function columnPath(column: Column): string {
	const { x, y, width, height } = column;
	if (height <= 0) return "";
	const radius = Math.min(4, width / 2, height);
	const bottom = y + height;
	return [
		`M${x},${bottom}`,
		`V${y + radius}`,
		`Q${x},${y} ${x + radius},${y}`,
		`H${x + width - radius}`,
		`Q${x + width},${y} ${x + width},${y + radius}`,
		`V${bottom}`,
		"Z",
	].join("");
}
