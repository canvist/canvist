/**
 * The messages between the page, the server realm (a worker), and browser
 * realms (frames). Every connection on the sandbox's `localhost` is its own
 * `MessagePort`, handed to the server realm with a {@link Dial}.
 */

export type LogLevel = "log" | "info" | "warn" | "error" | "debug";

/** A document the code made, as the page previews it. */
export interface DocSnapshot {
	/** Documents are numbered in the order the code made them, from 1. */
	readonly id: number;
	/** The extensions its engine registered, by name. */
	readonly extensions: readonly string[];
	readonly json: unknown;
	readonly html: string;
	readonly markdown: string;
}

export interface RunError {
	readonly message: string;
	/** The line in the file it came from, when the stack says. */
	readonly line?: number;
}

/** What a realm reports while code runs in it. */
export type RealmEvent =
	| { readonly type: "log"; readonly level: LogLevel; readonly text: string }
	/** The code's top-level statements finished. */
	| { readonly type: "settled"; readonly error?: RunError }
	/** Something failed after the top level finished, in a callback. */
	| { readonly type: "uncaught"; readonly error: RunError }
	| { readonly type: "listening"; readonly port: number }
	/** The documents the code made, every time one of them changes. */
	| { readonly type: "documents"; readonly documents: readonly DocSnapshot[] };

/** A connection to open on the sandbox's `localhost`. */
export type ConnectRequest =
	| {
		readonly kind: "websocket";
		readonly url: string;
		readonly protocols: readonly string[];
	}
	| {
		readonly kind: "http";
		readonly url: string;
		readonly method: string;
		readonly headers: readonly (readonly [string, string])[];
		readonly body: ArrayBuffer | null;
	};

/** A request for a connection, sent with the connection's port. */
export interface Dial {
	readonly type: "dial";
	/** The TCP port the request is for, as `listen` was given it. */
	readonly port: number;
	readonly request: ConnectRequest;
}

export type Frame = string | ArrayBuffer;

/** Messages the server side of a connection sends. */
export type ServerMessage =
	| { readonly type: "refused" }
	| { readonly type: "open"; readonly protocol: string }
	| { readonly type: "message"; readonly data: Frame }
	| { readonly type: "close"; readonly code: number; readonly reason: string }
	| {
		readonly type: "response";
		readonly status: number;
		readonly statusText: string;
		readonly headers: readonly (readonly [string, string])[];
		readonly body: ArrayBuffer;
	};

/** Messages the client side of a WebSocket sends. */
export type ClientMessage =
	| { readonly type: "message"; readonly data: Frame }
	| { readonly type: "close"; readonly code: number; readonly reason: string };

/** What the page asks of the server realm. */
export type WorkerCommand =
	| { readonly type: "run"; readonly file: string; readonly source: string }
	| Dial;

/** What the page asks of a browser realm: to run its code, or to close its connections before it goes. */
export type FrameCommand = FrameRun | { readonly type: "close" };

export interface FrameRun {
	readonly type: "run";
	readonly file: string;
	readonly source: string;
	/** Markup for the frame's body, before the code runs. */
	readonly html: string;
	/** The page's colour scheme, so the frame matches it. */
	readonly scheme: "light" | "dark";
}

/** What a browser realm sends the page: its events, and its dials. */
export type FrameMessage =
	| { readonly type: "ready" }
	/** The frame's content height, so the page can fit the frame to it. */
	| { readonly type: "resize"; readonly height: number }
	| { readonly type: "event"; readonly event: RealmEvent }
	| Dial;

/** Hosts the sandbox treats as its own machine. */
const LOCAL_HOSTS = new Set(["localhost", "127.0.0.1", "0.0.0.0", "[::1]"]);

/** The sandbox port `url` is for, or `undefined` when it leaves the sandbox. */
export function sandboxPort(url: URL): number | undefined {
	if (!LOCAL_HOSTS.has(url.hostname)) return undefined;
	if (url.port !== "") return Number(url.port);
	return url.protocol === "https:" || url.protocol === "wss:" ? 443 : 80;
}
