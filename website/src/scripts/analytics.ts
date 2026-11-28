/**
 * First-party analytics: a page view per page, and a click on anything
 * marked `data-capture="name"`, sent to canvist's own server at
 * `/v1/events` (services/cloud/src/analytics). Nothing goes to a third party.
 *
 * - The visitor is a random id kept in localStorage; a session ends after
 *   30 idle minutes. Without storage, each page is its own visitor.
 * - Query strings are never sent. The source is the referring host (other
 *   sites only) or `utm_source`, read on the page and sent alone.
 * - Nothing is sent when the browser asks not to be tracked (Global Privacy
 *   Control or Do Not Track), or from the admin dashboard.
 *
 * Extra properties ride on `data-capture-*` attributes:
 * `<a data-capture="plan chosen" data-capture-plan="builder">`.
 */

declare global {
	interface Navigator {
		/** Global Privacy Control; not in TypeScript's DOM types yet. */
		readonly globalPrivacyControl?: boolean;
	}
}

const ENDPOINT = "/v1/events";
const VISITOR_KEY = "canvist-visitor";
const SESSION_KEY = "canvist-session";
const SESSION_IDLE_MS = 30 * 60_000;
/** The server keeps sources up to this many characters. */
const MAX_SOURCE = 100;
const CAPTURE_PREFIX = "capture";

type Properties = Record<string, string>;

interface StoredSession {
	id: string;
	last: number;
}

/** Ids for this page, used when storage is blocked or holds nothing yet. */
const pageIds = { visitor: crypto.randomUUID(), session: crypto.randomUUID() };

function optedOut(): boolean {
	return navigator.globalPrivacyControl === true ||
		navigator.doNotTrack === "1";
}

function read(key: string): string | null {
	try {
		return localStorage.getItem(key);
	} catch {
		// Blocked storage reads as empty: the page's own ids stand in.
		return null;
	}
}

function write(key: string, value: string): void {
	try {
		localStorage.setItem(key, value);
	} catch {
		// Blocked storage keeps nothing: the ids then last one page.
	}
}

function visitor(): string {
	const stored = read(VISITOR_KEY);
	if (stored) return stored;

	write(VISITOR_KEY, pageIds.visitor);

	return pageIds.visitor;
}

function isSession(value: unknown): value is StoredSession {
	return typeof value === "object" && value !== null && "id" in value &&
		"last" in value &&
		typeof value.id === "string" && typeof value.last === "number";
}

function storedSession(): StoredSession | null {
	try {
		const stored: unknown = JSON.parse(read(SESSION_KEY) ?? "null");

		return isSession(stored) ? stored : null;
	} catch {
		// Anything unreadable starts a new session.
		return null;
	}
}

/** The current session, extended to `now`; a new one after 30 idle minutes. */
function session(now: number): string {
	const stored = storedSession();
	const id = stored && now - stored.last < SESSION_IDLE_MS
		? stored.id
		: pageIds.session;
	write(SESSION_KEY, JSON.stringify({ id, last: now } satisfies StoredSession));

	return id;
}

/** The referring site's host, unless the visit came from this site. */
function referrer(): string | null {
	try {
		const host = new URL(document.referrer).hostname;

		return host === location.hostname ? null : host;
	} catch {
		// No referrer, or one that is not a URL: a direct visit.
		return null;
	}
}

/** Where this visit came from: a campaign, another site, or nowhere. */
function source(): string | null {
	const campaign = new URLSearchParams(location.search).get("utm_source");

	return campaign ? campaign.slice(0, MAX_SOURCE) : referrer();
}

function send(name: string, properties: Properties): void {
	const now = Date.now();
	const event = {
		uuid: crypto.randomUUID(),
		name,
		visitor: visitor(),
		session: session(now),
		timestamp: now,
		path: location.pathname,
		source: source(),
		properties,
	};
	const body = JSON.stringify({ sent_at: now, events: [event] });

	// A beacon survives the page unloading after a click on a link.
	if (
		navigator.sendBeacon?.(
			ENDPOINT,
			new Blob([body], { type: "application/json" }),
		)
	) return;

	const request = {
		method: "POST",
		body,
		keepalive: true,
		headers: { "Content-Type": "application/json" },
	};
	// Analytics never interrupt the page: a lost event is only a lost count.
	fetch(ENDPOINT, request).catch(() => undefined);
}

/** `data-capture-plan="builder"` becomes `{ plan: "builder" }`. */
function captureProperties(element: HTMLElement): Properties {
	const entries = Object.entries(element.dataset)
		.filter(([key]) => key !== CAPTURE_PREFIX && key.startsWith(CAPTURE_PREFIX))
		.map(([key, value]) => {
			const name = key.slice(CAPTURE_PREFIX.length);

			return [name.charAt(0).toLowerCase() + name.slice(1), value ?? ""];
		});

	return Object.fromEntries(entries);
}

function captureClick(event: MouseEvent): void {
	const target = event.target instanceof Element
		? event.target.closest<HTMLElement>("[data-capture]")
		: null;
	const name = target?.dataset["capture"];
	if (target && name) send(name, captureProperties(target));
}

if (!optedOut() && !location.pathname.startsWith("/admin")) {
	send("$pageview", {});
	document.addEventListener("click", captureClick, { capture: true });
}

export {};
