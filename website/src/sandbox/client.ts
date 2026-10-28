import {
	type ConnectRequest,
	type Frame,
	sandboxPort,
	type ServerMessage,
} from "./protocol.ts";

/**
 * The sandbox port `url` is for. The page's own host is never one: in
 * development the site itself is served from `localhost`, and the realms
 * load their modules from it.
 */
function routedPort(url: URL): number | undefined {
	return url.host === globalThis.location?.host ? undefined : sandboxPort(url);
}

/**
 * `WebSocket` and `fetch` for code in a realm: requests to `localhost` open
 * a connection on the sandbox's network, and everything else goes to the
 * real one. They follow the web platform's own classes closely enough for
 * `@canvist/web/collab`, y-websocket, and hand-written code.
 */

/** Opens a connection to the sandbox's `port`; the server answers on the returned port. */
export type Dialer = (port: number, request: ConnectRequest) => MessagePort;

const ReadyState = { Connecting: 0, Open: 1, Closing: 2, Closed: 3 } as const;
type ReadyState = (typeof ReadyState)[keyof typeof ReadyState];

/**
 * A browser `WebSocket` whose `localhost` connections stay in the sandbox.
 * Its open sandbox sockets are kept in `open`, when given, so a realm can
 * close them all before it goes away.
 */
export function sandboxWebSocket(
	dial: Dialer,
	native: typeof WebSocket | undefined,
	open?: Set<{ close(code?: number, reason?: string): void }>,
): typeof WebSocket {
	class SandboxWebSocket extends EventTarget {
		static readonly CONNECTING = ReadyState.Connecting;
		static readonly OPEN = ReadyState.Open;
		static readonly CLOSING = ReadyState.Closing;
		static readonly CLOSED = ReadyState.Closed;
		readonly CONNECTING = ReadyState.Connecting;
		readonly OPEN = ReadyState.Open;
		readonly CLOSING = ReadyState.Closing;
		readonly CLOSED = ReadyState.Closed;

		readonly url: string;
		readonly extensions = "";
		readonly bufferedAmount = 0;
		binaryType: BinaryType = "blob";
		protocol = "";
		readyState: ReadyState = ReadyState.Connecting;
		onopen: ((event: Event) => void) | null = null;
		onmessage: ((event: MessageEvent) => void) | null = null;
		onerror: ((event: Event) => void) | null = null;
		onclose: ((event: CloseEvent) => void) | null = null;

		readonly #port: MessagePort | undefined;
		/** Blobs are read before they're sent, so later frames wait for them. */
		#sending: Promise<void> = Promise.resolve();

		constructor(url: string | URL, protocols: string | string[] = []) {
			super();
			const parsed = new URL(url, globalThis.location?.href);
			if (parsed.protocol === "http:") parsed.protocol = "ws:";
			if (parsed.protocol === "https:") parsed.protocol = "wss:";
			this.url = parsed.href;
			const port = routedPort(parsed);
			if (port === undefined) {
				if (native === undefined) {
					throw new DOMException(
						`Can't reach ${parsed.host} from the sandbox`,
						"SyntaxError",
					);
				}
				// Leaves the sandbox: the platform's own socket takes over.
				return new native(url, protocols) as unknown as SandboxWebSocket;
			}
			this.#port = dial(port, {
				kind: "websocket",
				url: this.url,
				protocols: typeof protocols === "string" ? [protocols] : protocols,
			});
			this.#port.onmessage = (event: MessageEvent<ServerMessage>) =>
				this.#receive(event.data);
			open?.add(this);
		}

		send(data: string | ArrayBufferLike | Blob | ArrayBufferView): void {
			if (this.readyState === ReadyState.Connecting) {
				throw new DOMException(
					"The socket is still connecting",
					"InvalidStateError",
				);
			}
			if (this.readyState !== ReadyState.Open) return;
			this.#sending = this.#sending.then(async () => {
				const frame = await frameOf(data);
				if (this.readyState === ReadyState.Open) {
					this.#post({ type: "message", data: frame }, frame);
				}
			});
		}

		close(code = 1000, reason = ""): void {
			if (this.readyState >= ReadyState.Closing) return;
			this.readyState = ReadyState.Closing;
			this.#post({ type: "close", code, reason });
			this.#closed(code, reason, true);
		}

		#post(
			message: { type: "message"; data: Frame } | {
				type: "close";
				code: number;
				reason: string;
			},
			frame?: Frame,
		): void {
			this.#port?.postMessage(
				message,
				frame instanceof ArrayBuffer ? [frame] : [],
			);
		}

		#receive(message: ServerMessage): void {
			switch (message.type) {
				case "open":
					this.protocol = message.protocol;
					this.readyState = ReadyState.Open;
					this.#emit(new Event("open"), this.onopen);
					return;
				case "message": {
					if (this.readyState !== ReadyState.Open) return;
					const data = typeof message.data === "string" ||
							this.binaryType === "arraybuffer"
						? message.data
						: new Blob([message.data]);
					this.#emit(
						new MessageEvent("message", {
							data,
							origin: new URL(this.url).origin,
						}),
						this.onmessage,
					);
					return;
				}
				case "close":
					this.#closed(message.code, message.reason, true);
					return;
				case "refused":
				case "response":
					// Nothing listens there, or it answered as a plain HTTP server.
					this.#emit(new Event("error"), this.onerror);
					this.#closed(1006, "", false);
			}
		}

		#closed(code: number, reason: string, wasClean: boolean): void {
			if (this.readyState === ReadyState.Closed) return;
			this.readyState = ReadyState.Closed;
			open?.delete(this);
			this.#port?.close();
			// Like the platform's, the close event comes in a later task.
			setTimeout(() =>
				this.#emit(
					new CloseEvent("close", { code, reason, wasClean }),
					this.onclose,
				)
			);
		}

		#emit<E extends Event>(
			event: E,
			handler: ((event: E) => void) | null,
		): void {
			this.dispatchEvent(event);
			handler?.call(this, event);
		}
	}
	return SandboxWebSocket as unknown as typeof WebSocket;
}

async function frameOf(
	data: string | ArrayBufferLike | Blob | ArrayBufferView,
): Promise<Frame> {
	if (typeof data === "string") return data;
	if (data instanceof Blob) return data.arrayBuffer();
	if (ArrayBuffer.isView(data)) {
		return new Uint8Array(data.buffer, data.byteOffset, data.byteLength).slice()
			.buffer;
	}
	return new Uint8Array(data).slice().buffer;
}

/** Statuses whose responses can't have a body. */
const NULL_BODY = new Set([101, 103, 204, 205, 304]);

/** A `fetch` whose requests to `localhost` go to the sandbox's servers. */
export function sandboxFetch(dial: Dialer, native: typeof fetch): typeof fetch {
	return async (input, init) => {
		const request = new Request(input, init);
		const url = new URL(request.url);
		const port = routedPort(url);
		if (port === undefined) return native(request);

		const body = request.method === "GET" || request.method === "HEAD"
			? null
			: await request.arrayBuffer();
		const channel = dial(port, {
			kind: "http",
			url: url.href,
			method: request.method,
			headers: [...request.headers],
			body,
		});
		const message = await new Promise<ServerMessage>((resolve) => {
			channel.onmessage = (event: MessageEvent<ServerMessage>) =>
				resolve(event.data);
		});
		channel.close();
		if (message.type !== "response") {
			throw new TypeError(
				`Failed to fetch ${url.href}: nothing is listening on port ${port} in the sandbox`,
			);
		}
		const status = message.status < 200 ? 502 : message.status;
		return new Response(NULL_BODY.has(status) ? null : message.body, {
			status,
			statusText: message.statusText,
			headers: message.headers.map((
				[name, value],
			): [string, string] => [name, value]),
		});
	};
}
