/**
 * The analytics tab: headline tiles against the period before, visitors
 * per day, and the top lists.
 */

import {
	type Column,
	columnPath,
	columns,
	type Count,
	type Day,
	formatChange,
	formatCount,
	formatDay,
	formatSeconds,
	formatShare,
	niceCeiling,
	type Report,
	type Summary,
} from "../../lib/admin.ts";
import { h, need, svg } from "./dom.ts";

/** The chart's drawing size, in SVG units; it scales to its box. */
const CHART = {
	width: 720,
	height: 220,
	left: 44,
	bottom: 28,
	top: 8,
} as const;

interface Tile {
	label: string;
	value: (summary: Summary) => number;
	format: (value: number) => string;
}

const TILES: Tile[] = [
	{
		label: "Visitors",
		value: (summary) => summary.visitors,
		format: formatCount,
	},
	{
		label: "Sessions",
		value: (summary) => summary.sessions,
		format: formatCount,
	},
	{
		label: "Page views",
		value: (summary) => summary.pageviews,
		format: formatCount,
	},
	{
		label: "Bounce rate",
		value: (summary) => summary.bounce_rate,
		format: formatShare,
	},
	{
		label: "Time per session",
		value: (summary) => summary.average_session_seconds,
		format: formatSeconds,
	},
];

type ListKey =
	| "pages"
	| "sources"
	| "devices"
	| "browsers"
	| "site_events"
	| "server_events";

const LISTS: { key: ListKey; title: string; empty: string }[] = [
	{ key: "pages", title: "Pages", empty: "No page views yet." },
	{ key: "sources", title: "Sources", empty: "No sessions yet." },
	{ key: "devices", title: "Devices", empty: "No page views yet." },
	{ key: "browsers", title: "Browsers", empty: "No page views yet." },
	{
		key: "site_events",
		title: "Clicks on the site",
		empty: "Nothing marked data-capture was clicked.",
	},
	{
		key: "server_events",
		title: "Product events",
		empty: "No projects or checkouts yet.",
	},
];

function comparison(change: string, days: number): string {
	if (change === "new") return `None in the ${days} days before`;

	return `${
		change === "no change" ? "No change" : change
	} on the ${days} days before`;
}

function tiles(report: Report, days: number): HTMLElement {
	return h(
		"dl",
		{ class: "tiles" },
		...TILES.map((tile) => {
			const current = tile.value(report.summary);
			const previous = tile.value(report.previous);
			return h(
				"div",
				{ class: "tile" },
				h("dt", { class: "tile__label" }, tile.label),
				h("dd", { class: "tile__value" }, tile.format(current)),
				h(
					"dd",
					{ class: "tile__change" },
					comparison(formatChange(current, previous), days),
				),
			);
		}),
	);
}

function axis(
	ceiling: number,
	plotWidth: number,
	plotHeight: number,
): SVGGElement {
	return svg(
		"g",
		{ class: "chart__axis", "aria-hidden": "true" },
		...[0, ceiling / 2, ceiling].map((tick) => {
			const y = CHART.top + plotHeight - (tick / ceiling) * plotHeight;
			return svg(
				"g",
				{},
				svg("line", {
					x1: CHART.left,
					x2: CHART.left + plotWidth,
					y1: y,
					y2: y,
				}),
				svg(
					"text",
					{ x: CHART.left - 8, y: y + 4, "text-anchor": "end" },
					formatCount(tick),
				),
			);
		}),
	);
}

function dayLabels(days: Day[], placed: Column[]): SVGGElement {
	const picks = [0, Math.floor((days.length - 1) / 2), days.length - 1];
	return svg(
		"g",
		{ class: "chart__days", "aria-hidden": "true" },
		...[...new Set(picks)].map((index) => {
			const column = placed[index];
			if (!column) return null;
			return svg(
				"text",
				{
					x: CHART.left + column.x + column.width / 2,
					y: CHART.height - 8,
					"text-anchor": "middle",
				},
				formatDay(column.day.day),
			);
		}),
	);
}

const PLOT = {
	width: CHART.width - CHART.left,
	height: CHART.height - CHART.bottom - CHART.top,
} as const;

/** Fill `tooltip` with `column`'s day and move it above the column. */
function showDay(tooltip: HTMLElement, column: Column): void {
	const center = CHART.left + column.x + column.width / 2;
	tooltip.replaceChildren(
		h("strong", {}, formatDay(column.day.day)),
		h("span", {}, `${formatCount(column.day.visitors)} visitors`),
		h("span", {}, `${formatCount(column.day.pageviews)} page views`),
	);
	tooltip.style.left = `${(center / CHART.width) * 100}%`;
	tooltip.hidden = false;
}

/** A column, and a hit area as tall as the plot so thin days are easy to hover. */
function mark(column: Column, band: number, tooltip: HTMLElement): SVGGElement {
	const hit = svg("rect", {
		class: "chart__hit",
		x: CHART.left + column.x - (band - column.width) / 2,
		y: CHART.top,
		width: band,
		height: PLOT.height,
	});
	const shape = columnPath({
		...column,
		x: CHART.left + column.x,
		y: CHART.top + column.y,
	});
	hit.addEventListener("pointerenter", () => showDay(tooltip, column));

	return svg("g", {}, svg("path", { class: "chart__column", d: shape }), hit);
}

/** Visitors per day as columns, with a tooltip per day on hover. */
function chart(days: Day[]): HTMLElement {
	const ceiling = niceCeiling(Math.max(...days.map((day) => day.visitors), 0));
	const placed = columns(days, PLOT.width, PLOT.height, ceiling);
	const band = PLOT.width / Math.max(days.length, 1);
	const total = days.reduce((sum, day) => sum + day.visitors, 0);
	const tooltip = h("div", {
		class: "chart__tooltip",
		role: "presentation",
		hidden: true,
	});
	const label = `Visitors per day: ${
		formatCount(total)
	} in all. The table below lists each day.`;
	const drawing = svg(
		"svg",
		{
			viewBox: `0 0 ${CHART.width} ${CHART.height}`,
			role: "img",
			"aria-label": label,
		},
		axis(ceiling, PLOT.width, PLOT.height),
		...placed.map((column) => mark(column, band, tooltip)),
		dayLabels(days, placed),
	);
	drawing.addEventListener("pointerleave", () => {
		tooltip.hidden = true;
	});

	return h(
		"figure",
		{ class: "chart" },
		h("figcaption", { class: "panel-title" }, "Visitors per day"),
		h("div", { class: "chart__plot" }, drawing, tooltip),
		dayTable(days),
	);
}

function dayTable(days: Day[]): HTMLElement {
	return h(
		"details",
		{ class: "chart__table" },
		h("summary", {}, "Show each day"),
		h(
			"table",
			{},
			h(
				"thead",
				{},
				h(
					"tr",
					{},
					h("th", { scope: "col" }, "Day"),
					h("th", { scope: "col" }, "Visitors"),
					h("th", { scope: "col" }, "Page views"),
				),
			),
			h(
				"tbody",
				{},
				...days.map((day) =>
					h(
						"tr",
						{},
						h("th", { scope: "row" }, formatDay(day.day)),
						h("td", {}, formatCount(day.visitors)),
						h("td", {}, formatCount(day.pageviews)),
					)
				),
			),
		),
	);
}

/** One top list: each row's share of the leader as a thin bar. */
function topList(title: string, rows: Count[], empty: string): HTMLElement {
	const most = Math.max(...rows.map((row) => row.count), 1);
	const body = rows.length === 0 ? h("p", { class: "empty" }, empty) : h(
		"table",
		{ class: "top" },
		h(
			"thead",
			{},
			h(
				"tr",
				{},
				h("th", { scope: "col" }, "Name"),
				h("th", { scope: "col" }, "Count"),
				h("th", { scope: "col" }, "Visitors"),
			),
		),
		h(
			"tbody",
			{},
			...rows.map((row) =>
				h(
					"tr",
					{ style: `--share: ${(row.count / most) * 100}%` },
					h(
						"th",
						{ scope: "row" },
						h("span", { class: "top__key" }, row.key || "(none)"),
					),
					h("td", {}, formatCount(row.count)),
					h("td", {}, formatCount(row.visitors)),
				)
			),
		),
	);
	return h(
		"section",
		{ class: "panel" },
		h("h3", { class: "panel-title" }, title),
		body,
	);
}

/** Draw `report`, covering `days` days, into `root`. */
export function renderReport(
	root: HTMLElement,
	report: Report,
	days: number,
): void {
	const target = need<HTMLElement>(root, "[data-report]");
	const lists = LISTS.map(({ key, title, empty }) =>
		topList(title, report[key], empty)
	);
	const children: Node[] = [
		tiles(report, days),
		chart(report.days),
		h("div", { class: "lists" }, ...lists),
	];
	if (report.truncated) {
		children.push(
			h(
				"p",
				{ class: "note", role: "note" },
				"This period has more events than one report reads, so these counts are low.",
			),
		);
	}

	delete target.dataset["tone"];
	target.replaceChildren(...children);
}
