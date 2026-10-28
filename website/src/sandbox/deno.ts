import type { Connection, Network, UpgradeIncoming } from "./network.ts";

/**
 * `Deno.serve` and `Deno.upgradeWebSocket` on the sandbox network, so the
 * servers the docs show for Deno run here unchanged: a fetch-style handler
 * gets a `Request` and returns a `Response`, and a WebSocket upgrade
 * returns the response `upgradeWebSocket` made.
 */

type Handler = (
	request: Request,
	info: ServeInfo,
) => Response | Promise<Response>;

interface ServeInfo {
	readonly remoteAddr: {
		readonly transport: "tcp";
		readonly hostname: string;
		readonly port: number;
	};
	readonly completed: Promise<void>;
}

interface ServeOptions {
	readonly port?: number;
	readonly hostname?: string;
	readonly signal?: AbortSignal;
	readonly onListen?: (
		address: { hostname: string; port: number; transport: "tcp" },
	) => void;
	readonly onError?: (error: unknown) => Response | Promise<Response>;
	readonly handler?: Handler;
}

/** A server-side WebSocket, shaped like Deno's (the browser's `WebSocket`). */
class ServerWebSocket extends EventTarget {
	readonly CONNECTING = 0;
	readonly OPEN = 1;
	readonly CLOSING = 2;
	readonly CLOSED = 3;
	readonly url: string;
	binaryType: BinaryType = "blob";
	readyState = 0;
	protocol = "";
	onopen: ((event: Event) => void) | null = null;
	onmessage: ((event: MessageEvent) => void) | null = null;
	onerror: ((event: Event) => void) | null = null;
	onclose: ((event: CloseEvent) => void) | null = null;
	#connection: Connection | undefined;

	constructor(url: string) {
		super();
		this.url = url;
	}

	/** @internal The handshake finished. */
	attach(connection: Connection, protocol: string): void {
		this.#connection = connection;
		this.protocol = protocol;
		this.readyState = this.OPEN;
		connection.onmessage = (frame) => {
			const data =
				typeof frame === "string" || this.binaryType === "arraybuffer"
					? frame
					: new Blob([frame]);
			this.#emit(new MessageEvent("message", { data }), this.onmessage);
		};
		connection.onclose = (code, reason) => {
			this.readyState = this.CLOSED;
			this.#emit(
				new CloseEvent("close", { code, reason, wasClean: true }),
				this.onclose,
			);
		};
		this.#emit(new Event("open"), this.onopen);
	}

	send(data: string | ArrayBufferLike | ArrayBufferView): void {
		if (this.#connection === undefined) {
			throw new DOMException(
				"The socket is still connecting",
				"InvalidStateError",
			);
		}
		if (typeof data === "string") this.#connection.send(data);
		else if (ArrayBuffer.isView(data)) {
			this.#connection.send(
				new Uint8Array(data.buffer, data.byteOffset, data.byteLength).slice()
					.buffer,
			);
		} else this.#connection.send(new Uint8Array(data).slice().buffer);
	}

	close(code = 1000, reason = ""): void {
		this.#connection?.close(code, reason);
	}

	#emit<E extends Event>(event: E, handler: ((event: E) => void) | null): void {
		this.dispatchEvent(event);
		handler?.call(this, event);
	}
}

interface Server {
	readonly addr: {
		readonly transport: "tcp";
		readonly hostname: string;
		readonly port: number;
	};
	readonly finished: Promise<void>;
	shutdown(): Promise<void>;
	ref(): void;
	unref(): void;
}

type ServeArgs =
	| [handler: Handler]
	| [options: ServeOptions, handler: Handler]
	| [options: ServeOptions & { readonly handler: Handler }];

export interface DenoNamespace {
	serve(handler: Handler): Server;
	serve(options: ServeOptions, handler: Handler): Server;
	serve(options: ServeOptions & { readonly handler: Handler }): Server;
	upgradeWebSocket(
		request: Request,
		options?: { protocol?: string },
	): { socket: WebSocket; response: Response };
	readonly env: {
		get(name: string): string | undefined;
		set(name: string, value: string): void;
		toObject(): Record<string, string>;
	};
	readonly args: readonly string[];
}

export function denoNamespace(
	network: Network,
	env: Record<string, string>,
	report: (error: unknown) => void,
): DenoNamespace {
	const pending = new WeakMap<Request, UpgradeIncoming>();
	const upgraded = new WeakMap<
		Response,
		{ socket: ServerWebSocket; protocol: string }
	>();

	return {
		serve(...args: ServeArgs): Server {
			const [first, second] = args;
			const options: ServeOptions = typeof first === "function"
				? { handler: first }
				: first;
			const handler = second ?? options.handler;
			if (handler === undefined) {
				throw new TypeError("Deno.serve needs a handler");
			}

			let finish: () => void = () => {};
			const finished = new Promise<void>((resolve) => {
				finish = resolve;
			});
			const listening = network.listen(options.port ?? 8000, (incoming) => {
				if (incoming.kind === "websocket") {
					pending.set(incoming.request, incoming);
				}
				const info: ServeInfo = {
					remoteAddr: { transport: "tcp", hostname: "127.0.0.1", port: 0 },
					completed: Promise.resolve(),
				};
				void (async () => {
					let response: Response;
					try {
						response = await handler(incoming.request, info);
					} catch (error) {
						report(error);
						response = options.onError
							? await options.onError(error)
							: new Response("Internal Server Error", { status: 500 });
					}
					if (incoming.kind === "http") {
						await incoming.respond(response);
						return;
					}
					const upgrade = upgraded.get(response);
					if (upgrade) {
						upgrade.socket.attach(
							incoming.accept(upgrade.protocol),
							upgrade.protocol,
						);
					} else incoming.refuse(response.status);
				})();
			});
			const addr = {
				transport: "tcp" as const,
				hostname: options.hostname ?? "0.0.0.0",
				port: listening.port,
			};
			const shutdown = () => {
				listening.close();
				finish();
				return finished;
			};
			options.signal?.addEventListener("abort", () => void shutdown());
			if (options.onListen) options.onListen(addr);
			else console.log(`Listening on http://localhost:${listening.port}/`);
			return { addr, finished, shutdown, ref() {}, unref() {} };
		},

		upgradeWebSocket(request, options = {}) {
			const incoming = pending.get(request);
			if (incoming === undefined) {
				throw new TypeError(
					"Invalid Header: 'upgrade' header must contain 'websocket'",
				);
			}
			pending.delete(request);
			const socket = new ServerWebSocket(request.url.replace(/^http/, "ws"));
			// Deno answers the handshake with this response; here it marks the upgrade.
			const response = new Response(null, { status: 200 });
			upgraded.set(response, { socket, protocol: options.protocol ?? "" });
			return { socket: socket as unknown as WebSocket, response };
		},

		env: {
			get: (name) => env[name],
			set: (name, value) => {
				env[name] = value;
			},
			toObject: () => ({ ...env }),
		},
		args: [],
	};
}
