/// <reference lib="dom" />

/**
 * Measures what each page costs a visitor: the bytes on the wire, the
 * requests, when it paints, how long the main thread is blocked, and, on the
 * playground, how long the editor takes to start and to paint a keystroke.
 *
 *   deno run -A scripts/page-weight.ts [--url <origin>] [--out <file.json>]
 *     [--compare <base.json>] [--runs <n>] [--throttle]
 *
 * `--url` defaults to a local server over `dist/` with the production
 * Caddyfile (`caddy` on PATH, as `deno task perf:serve` starts it), so the
 * encodings and cache headers are the ones visitors get. Point it at
 * https://canvist.dev to measure what is deployed.
 *
 * Every page is loaded in a fresh browser context, so nothing is cached, and
 * after load the page is scrolled to the bottom so lazy content (the landing
 * page's animations, the docs' live samples) is counted too.
 *
 * - **bytes**: compressed bytes on the wire (CDP `encodedDataLength`),
 *   headers included, split into document, script, wasm, font, image, and
 *   other.
 * - **fcp / lcp**: first and largest contentful paint, in milliseconds.
 * - **tbt**: total blocking time, the sum of every long task's time over
 *   50 ms, from load until the page is idle.
 * - **editor**: on pages with a canvist editor, the time from navigation
 *   until the editor is painted, and keystroke-to-paint p50 / p95 over 50
 *   keys, from each key event until the frame after it.
 *
 * `--throttle` emulates a mid-range phone on a fast 4G link (4× CPU
 * slowdown, 9 Mbit/s down, 40 ms RTT), which is what Lighthouse's mobile
 * preset approximates. Timings are the median of `--runs` loads (default 3);
 * bytes are deterministic and come from the first.
 */
import { type CDPSession, chromium, type Page } from "@playwright/test";
import { parseArgs } from "node:util";

const { values } = parseArgs({
	args: Deno.args,
	options: {
		url: { type: "string", default: "http://127.0.0.1:4810" },
		out: { type: "string" },
		compare: { type: "string" },
		runs: { type: "string", default: "3" },
		pages: { type: "string" },
		throttle: { type: "boolean", default: false },
	},
});

/** The pages people land on, one of each layout. */
const DEFAULT_PAGES = [
	"/",
	"/pricing",
	"/services/sync",
	"/docs/concepts/how-it-works",
	"/playground",
];

const origin = values.url.replace(/\/$/, "");
const pages = values.pages?.split(",") ?? DEFAULT_PAGES;
const runs = Number(values.runs);

type Kind = "document" | "script" | "wasm" | "font" | "image" | "other";
const KINDS: readonly Kind[] = [
	"document",
	"script",
	"wasm",
	"font",
	"image",
	"other",
];

export interface PageResult {
	page: string;
	requests: number;
	bytes: Record<Kind | "total", number>;
	fcp: number;
	lcp: number;
	tbt: number;
	editorReady?: number;
	keyP50?: number;
	keyP95?: number;
	/** The five largest responses: path, kind, bytes, encoding. */
	largest: { path: string; kind: Kind; bytes: number; encoding: string }[];
}

function kindOf(url: string, type: string, mime: string): Kind {
	if (type === "Document") return "document";
	if (mime.includes("wasm") || url.endsWith(".wasm")) return "wasm";
	if (type === "Script" || mime.includes("javascript")) return "script";
	if (type === "Font" || /\.woff2?$/.test(url)) return "font";
	if (type === "Image") return "image";
	return "other";
}

function median(values: number[]): number {
	const sorted = [...values].sort((a, b) => a - b);
	return sorted[Math.floor(sorted.length / 2)] ?? 0;
}

function percentile(values: number[], p: number): number {
	const sorted = [...values].sort((a, b) => a - b);
	return sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * p))] ??
		0;
}

async function throttle(cdp: CDPSession): Promise<void> {
	await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
	await cdp.send("Network.emulateNetworkConditions", {
		offline: false,
		latency: 40,
		downloadThroughput: (9 * 1024 * 1024) / 8,
		uploadThroughput: (1.5 * 1024 * 1024) / 8,
	});
}

/** Collects every response's size and kind until `stop` is called. */
function recordNetwork(cdp: CDPSession) {
	const responses = new Map<
		string,
		{ url: string; kind: Kind; encoding: string; bytes: number }
	>();
	cdp.on("Network.responseReceived", (event) => {
		const headers = Object.fromEntries(
			Object.entries(event.response.headers).map((
				[key, value],
			) => [key.toLowerCase(), String(value)]),
		);
		responses.set(event.requestId, {
			url: event.response.url,
			kind: kindOf(event.response.url, event.type, event.response.mimeType),
			encoding: headers["content-encoding"] ?? "",
			bytes: 0,
		});
	});
	cdp.on("Network.loadingFinished", (event) => {
		const response = responses.get(event.requestId);
		if (response) response.bytes = event.encodedDataLength;
	});
	return responses;
}

/** Installs the observers before any page script runs. */
const OBSERVE = () => {
	const w = window as unknown as Record<string, unknown>;
	w["__lcp"] = 0;
	w["__tbt"] = 0;
	new PerformanceObserver((list) => {
		for (const entry of list.getEntries()) w["__lcp"] = entry.startTime;
	}).observe({ type: "largest-contentful-paint", buffered: true });
	new PerformanceObserver((list) => {
		for (const entry of list.getEntries()) {
			w["__tbt"] = (w["__tbt"] as number) + Math.max(0, entry.duration - 50);
		}
	}).observe({ type: "longtask", buffered: true });
	// The first frame with a laid-out editor canvas.
	const poll = () => {
		const canvas = document.querySelector<HTMLCanvasElement>(".cv-canvas");
		if (canvas && canvas.offsetHeight > 0 && canvas.width > 0) {
			w["__editorReady"] = performance.now();
		} else requestAnimationFrame(poll);
	};
	requestAnimationFrame(poll);
	// When the editor last drew, for keystroke-to-paint.
	const proto = CanvasRenderingContext2D.prototype as unknown as Record<
		string,
		(...args: unknown[]) => unknown
	>;
	for (const name of ["fillText", "fillRect", "drawImage", "putImageData"]) {
		const draw = proto[name]!;
		proto[name] = function (this: unknown, ...args: unknown[]) {
			w["__drawnAt"] = performance.now();
			return draw.apply(this, args);
		};
	}
};

async function settle(page: Page): Promise<void> {
	await page.waitForLoadState("networkidle");
	await page.evaluate(async () => {
		const step = innerHeight * 0.8;
		for (let y = 0; y < document.body.scrollHeight; y += step) {
			scrollTo(0, y);
			await new Promise((resolve) => setTimeout(resolve, 60));
		}
		scrollTo(0, 0);
	});
	await page.waitForLoadState("networkidle");
	// Rive and wasm modules finish compiling after their bytes arrive.
	await page.waitForTimeout(500);
}

/**
 * Time to a painted editor, and keystroke-to-paint over 50 keys: from each
 * keydown's timestamp until the editor's last canvas draw for it, which is
 * what `editor.inputLatency()` measures. The wait for the display's next
 * frame comes on top and depends on the display, not on canvist.
 */
async function measureEditor(
	page: Page,
): Promise<Pick<PageResult, "editorReady" | "keyP50" | "keyP95"> | undefined> {
	// The playground runs the editor in a sandboxed frame.
	const frame = await (async () => {
		for (const frame of page.frames()) {
			if (await frame.locator(".cv-canvas").count()) return frame;
		}
	})();
	if (!frame) return undefined;
	const canvas = frame.locator(".cv-canvas").first();
	// Measured from the page's navigation, whichever frame painted it.
	const origin = await page.evaluate(() => performance.timeOrigin);
	const editorReady = await frame.evaluate(
		(origin) =>
			performance.timeOrigin +
			((window as unknown as Record<string, number>)["__editorReady"] ?? 0) -
			origin,
		origin,
	);
	await canvas.click();
	await frame.evaluate(() => {
		const w = window as unknown as Record<string, number>;
		addEventListener("keydown", (event) => {
			w["__keyAt"] = event.timeStamp;
		}, { capture: true });
	});
	const latencies: number[] = [];
	for (let index = 0; index < 50; index++) {
		await page.keyboard.press("a");
		await page.waitForTimeout(30);
		latencies.push(
			await frame.evaluate(() => {
				const w = window as unknown as Record<string, number>;
				return (w["__drawnAt"] ?? 0) - (w["__keyAt"] ?? 0);
			}),
		);
	}
	if (!latencies.length) return { editorReady: Math.round(editorReady) };
	return {
		editorReady: Math.round(editorReady),
		keyP50: Number(median(latencies).toFixed(1)),
		keyP95: Number(percentile(latencies, 0.95).toFixed(1)),
	};
}

async function measurePage(path: string): Promise<PageResult> {
	// PLAYWRIGHT_CHROMIUM lets a machine whose browser doesn't match this
	// Playwright version point at the one it has.
	const browser = await chromium.launch({
		executablePath: Deno.env.get("PLAYWRIGHT_CHROMIUM") || undefined,
	});
	const samples: PageResult[] = [];
	try {
		for (let run = 0; run < runs; run++) {
			const context = await browser.newContext({
				viewport: { width: 1280, height: 800 },
			});
			const page = await context.newPage();
			const cdp = await context.newCDPSession(page);
			await cdp.send("Network.enable");
			await cdp.send("Network.setCacheDisabled", { cacheDisabled: true });
			if (values.throttle) await throttle(cdp);
			const responses = recordNetwork(cdp);
			await page.addInitScript(OBSERVE);
			await page.goto(origin + path, { waitUntil: "load", timeout: 120_000 });
			await settle(page);
			const timings = await page.evaluate(() => {
				const w = window as unknown as Record<string, number>;
				const fcp = performance.getEntriesByName("first-contentful-paint")[0];
				return {
					fcp: Math.round(fcp?.startTime ?? 0),
					lcp: Math.round(w["__lcp"] ?? 0),
					tbt: Math.round(w["__tbt"] ?? 0),
				};
			});
			const editor = await measureEditor(page);
			const list = [...responses.values()];
			const bytes = Object.fromEntries(
				[...KINDS, "total"].map((kind) => [kind, 0]),
			) as PageResult["bytes"];
			for (const response of list) {
				bytes[response.kind] += response.bytes;
				bytes.total += response.bytes;
			}
			samples.push({
				page: path,
				requests: list.length,
				bytes,
				...timings,
				...editor,
				largest: list.sort((a, b) => b.bytes - a.bytes).slice(0, 5).map((
					response,
				) => ({
					path: response.url.replace(origin, ""),
					kind: response.kind,
					bytes: response.bytes,
					encoding: response.encoding,
				})),
			});
			await context.close();
		}
	} finally {
		await browser.close();
	}
	const first = samples[0]!;
	const pick = (
		key: "fcp" | "lcp" | "tbt" | "editorReady" | "keyP50" | "keyP95",
	) => {
		const values = samples.map((sample) => sample[key]).filter((
			value,
		): value is number => value !== undefined);
		return values.length ? median(values) : undefined;
	};
	return {
		...first,
		fcp: pick("fcp")!,
		lcp: pick("lcp")!,
		tbt: pick("tbt")!,
		editorReady: pick("editorReady"),
		keyP50: pick("keyP50"),
		keyP95: pick("keyP95"),
	};
}

const kib = (bytes: number) => (bytes / 1024).toFixed(1);

function delta(head: number, base: number | undefined, unit = ""): string {
	if (base === undefined || base === head) return "";
	const change = head - base;
	const percent = base
		? ` ${change > 0 ? "+" : ""}${((change / base) * 100).toFixed(0)}%`
		: "";
	return ` (${change > 0 ? "+" : ""}${
		unit === "KiB" ? kib(change) : change
	}${percent})`;
}

function report(results: PageResult[], base?: PageResult[]): string {
	const lines = [
		`Measured ${origin}${
			values.throttle ? ", throttled (4× CPU, 9 Mbit/s, 40 ms RTT)" : ""
		}; times in ms, median of ${runs}.`,
		"",
		"| Page | Requests | Total KiB | Script | Wasm | Font | FCP | LCP | TBT | Editor ready | Key → paint p50 / p95 |",
		"| ---- | -------: | --------: | -----: | ---: | ---: | --: | --: | --: | -----------: | ------: |",
	];
	for (const result of results) {
		const before = base?.find((entry) => entry.page === result.page);
		lines.push(
			`| ${result.page} | ${result.requests}${
				delta(result.requests, before?.requests)
			} | ${kib(result.bytes.total)}${
				delta(result.bytes.total, before?.bytes.total, "KiB")
			} | ${kib(result.bytes.script)} | ${kib(result.bytes.wasm)} | ${
				kib(result.bytes.font)
			} | ${result.fcp} | ${result.lcp} | ${result.tbt} | ${
				result.editorReady ?? ""
			} | ${
				result.keyP50 === undefined ? "" : `${result.keyP50} / ${result.keyP95}`
			} |`,
		);
	}
	return lines.join("\n");
}

const results: PageResult[] = [];
for (const path of pages) results.push(await measurePage(path));

const base = values.compare
	? (JSON.parse(await Deno.readTextFile(values.compare)) as PageResult[])
	: undefined;
console.log(report(results, base));
for (const result of results) {
	console.log(`\n${result.page}, largest responses:`);
	for (const response of result.largest) {
		console.log(
			`  ${kib(response.bytes).padStart(7)} KiB  ${response.kind.padEnd(8)} ${
				(response.encoding || "-").padEnd(5)
			} ${response.path}`,
		);
	}
}
if (values.out) {
	await Deno.writeTextFile(
		values.out,
		JSON.stringify(results, null, "\t") + "\n",
	);
}
