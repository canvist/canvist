import { type Dialer, sandboxFetch } from "./client.ts";
import type {
	Dial,
	FrameCommand,
	FrameMessage,
	RealmEvent,
	ServerMessage,
	WorkerCommand,
} from "./protocol.ts";

/**
 * The page's side of the sandbox: one server realm (a worker) and any
 * number of browser realms (frames), with every `localhost` connection a
 * frame opens routed to the worker.
 *
 * Code only ever comes from the page that owns the sandbox, never from a
 * URL, so a link can't make the site run someone else's code.
 */

/** Which realm an event came from: the server, or the browser peer with this number. */
export type RealmSource = { readonly realm: "server" } | {
	readonly realm: "browser";
	readonly peer: number;
};

export interface BrowserRun {
	readonly file: string;
	readonly source: string;
	readonly html: string;
	/** The frame's accessible name. */
	readonly title: string;
	/** Query parameters for the frame's URL, which the code can read from `location.search`. */
	readonly params?: Readonly<Record<string, string>>;
}

/** A browser realm running in a frame. */
export interface BrowserPeer {
	readonly peer: number;
	readonly frame: HTMLIFrameElement;
	/** Resolves when the file's top level has finished. */
	readonly settled: Promise<void>;
	remove(): void;
}

const FRAME_URL = "/sandbox/frame";
/** How long a frame gets to close its connections before it's removed. */
const FRAME_CLOSE_MS = 80;

export class Sandbox {
	readonly #onEvent: (source: RealmSource, event: RealmEvent) => void;
	readonly #frames = new Map<
		Window,
		{
			peer: BrowserPeer;
			onMessage: (message: FrameMessage, ports: readonly MessagePort[]) => void;
		}
	>();
	#worker: Worker | undefined;
	#nextPeer = 1;
	/** Frames fit their content's height, unless the page sizes them. */
	readonly #fitFrames: boolean;

	constructor(
		onEvent: (source: RealmSource, event: RealmEvent) => void,
		options: { fitFrames?: boolean } = {},
	) {
		this.#onEvent = onEvent;
		this.#fitFrames = options.fitFrames ?? false;
		addEventListener("message", this.#onWindowMessage);
	}

	/** Whether a server realm is running. */
	get serving(): boolean {
		return this.#worker !== undefined;
	}

	/**
	 * Starts a fresh server realm running `source`, stopping the last one,
	 * and resolves when the file's top level has finished.
	 */
	runServer(file: string, source: string): Promise<void> {
		this.stopServer();
		const worker = new Worker(new URL("./server-worker.ts", import.meta.url), {
			type: "module",
			name: "canvist sandbox server",
		});
		this.#worker = worker;
		const settled = new Promise<void>((resolve) => {
			worker.addEventListener("message", (event: MessageEvent<RealmEvent>) => {
				if (this.#worker !== worker) return;
				this.#onEvent({ realm: "server" }, event.data);
				if (event.data.type === "settled") resolve();
			});
		});
		worker.addEventListener("error", (event) => {
			// A worker that fails to load reports here, not through its own handlers.
			this.#onEvent({ realm: "server" }, {
				type: "settled",
				error: { message: event.message || "The server realm failed to start" },
			});
		});
		this.#post(worker, { type: "run", file, source });
		return settled;
	}

	stopServer(): void {
		this.#worker?.terminate();
		this.#worker = undefined;
	}

	/** Runs browser code in a new frame inside `container`. */
	addBrowser(container: HTMLElement, run: BrowserRun): BrowserPeer {
		const frame = document.createElement("iframe");
		frame.src = run.params
			? `${FRAME_URL}?${new URLSearchParams(run.params)}`
			: FRAME_URL;
		frame.title = run.title;
		frame.className = "sandbox-frame";
		const number = this.#nextPeer++;
		const source: RealmSource = { realm: "browser", peer: number };
		let settle: () => void = () => {};
		const settled = new Promise<void>((resolve) => {
			settle = resolve;
		});
		const peer: BrowserPeer = {
			peer: number,
			frame,
			settled,
			remove: () => {
				const target = frame.contentWindow;
				if (target === null) return;
				this.#frames.delete(target);
				// Let the frame close its sockets first, so servers see the peer leave.
				const close: FrameCommand = { type: "close" };
				target.postMessage(close, location.origin);
				setTimeout(() => frame.remove(), FRAME_CLOSE_MS);
			},
		};
		container.append(frame);
		const target = frame.contentWindow;
		if (target === null) throw new Error("The sandbox frame has no window");

		this.#frames.set(target, {
			peer,
			onMessage: (message, ports) => {
				switch (message.type) {
					case "ready": {
						const command: FrameCommand = {
							type: "run",
							file: run.file,
							source: run.source,
							html: run.html,
							scheme: pageScheme(),
						};
						target.postMessage(command, location.origin);
						return;
					}
					case "resize":
						if (this.#fitFrames) {
							frame.style.height = `${Math.ceil(message.height)}px`;
						}
						return;
					case "dial":
						this.#route(message, ports[0]);
						return;
					case "event":
						this.#onEvent(source, message.event);
						if (message.event.type === "settled") settle();
				}
			},
		});
		return peer;
	}

	/** Opens a connection from the page itself to the server realm. */
	readonly dial: Dialer = (port, request) => {
		const channel = new MessageChannel();
		this.#route({ type: "dial", port, request }, channel.port2);
		return channel.port1;
	};

	/** `fetch` from the page itself, with `localhost` going to the server realm. */
	readonly fetch: typeof fetch = sandboxFetch(
		this.dial,
		fetch.bind(globalThis),
	);

	dispose(): void {
		this.stopServer();
		for (const { peer } of this.#frames.values()) peer.remove();
		removeEventListener("message", this.#onWindowMessage);
	}

	#route(dial: Dial, channel: MessagePort | undefined): void {
		if (channel === undefined) return;
		if (this.#worker === undefined) {
			const refused: ServerMessage = { type: "refused" };
			channel.postMessage(refused);
			channel.close();
			return;
		}
		this.#post(this.#worker, dial, [channel]);
	}

	#post(
		worker: Worker,
		command: WorkerCommand,
		transfer: Transferable[] = [],
	): void {
		worker.postMessage(command, transfer);
	}

	readonly #onWindowMessage = (event: MessageEvent<FrameMessage>) => {
		// A frame's window comes from another realm, so `instanceof Window` is false for it.
		if (event.origin !== location.origin || event.source === null) return;
		this.#frames.get(event.source as Window)?.onMessage(
			event.data,
			event.ports,
		);
	};
}

/** The colour scheme the page shows: pinned by the theme toggle, or the system's. */
export function pageScheme(): "light" | "dark" {
	const pinned = document.documentElement.dataset["theme"];
	if (pinned === "light" || pinned === "dark") return pinned;
	return matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}
