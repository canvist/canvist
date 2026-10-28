import type { Connection, Network, UpgradeIncoming } from "../network.ts";
import type { Frame } from "../protocol.ts";
import { Buffer, bufferOf } from "./buffer.ts";
import { EventEmitter } from "./events.ts";
import { type HttpServer, IncomingMessage, UpgradeSocket } from "./http.ts";

/**
 * The `ws` package's server, the way Node servers accept WebSockets:
 * `new WebSocketServer({ port })`, or `{ server }` to share a `node:http`
 * server, or `{ noServer: true }` with `handleUpgrade`. Messages arrive as
 * a `Buffer` and an `isBinary` flag, as `ws` delivers them.
 */

const OPEN = 1;
const CLOSED = 3;

export class WebSocket extends EventEmitter {
	static readonly CONNECTING = 0;
	static readonly OPEN = OPEN;
	static readonly CLOSING = 2;
	static readonly CLOSED = CLOSED;
	readonly CONNECTING = 0;
	readonly OPEN = OPEN;
	readonly CLOSING = 2;
	readonly CLOSED = CLOSED;
	readonly protocol: string;
	binaryType: "nodebuffer" | "arraybuffer" = "nodebuffer";
	readyState: number = OPEN;
	readonly #connection: Connection;

	/** @internal Sockets come from a {@link WebSocketServer}. */
	constructor(connection: Connection, protocol: string) {
		super();
		// Code written for Node may still try `new WebSocket(url)` as a client.
		if (
			typeof (connection as unknown) === "string" || connection instanceof URL
		) {
			throw new TypeError(
				"`new WebSocket(url)` from `ws` isn't in the sandbox: use the global `WebSocket` to connect",
			);
		}
		this.#connection = connection;
		this.protocol = protocol;
		connection.onmessage = (frame) => {
			const isBinary = typeof frame !== "string";
			const data =
				this.binaryType === "arraybuffer" && typeof frame !== "string"
					? frame
					: bufferOf(frame);
			this.emit("message", data, isBinary);
		};
		connection.onclose = (code, reason) => {
			this.readyState = CLOSED;
			this.emit("close", code, Buffer.from(reason));
		};
	}

	send(
		data: string | ArrayBuffer | ArrayBufferView,
		options?: { binary?: boolean } | (() => void),
		callback?: () => void,
	): void {
		const done = typeof options === "function" ? options : callback;
		if (this.readyState !== OPEN) {
			done?.();
			return;
		}
		this.#connection.send(
			frameOf(data, typeof options === "object" ? options.binary : undefined),
		);
		done?.();
	}

	close(code = 1000, reason = ""): void {
		this.#connection.close(code, reason);
	}

	terminate(): void {
		this.#connection.close(1006, "");
	}

	/** Pings need no answer here: the sandbox's connections never go stale. */
	ping(): void {}

	pong(): void {}
}

function frameOf(
	data: string | ArrayBuffer | ArrayBufferView,
	binary: boolean | undefined,
): Frame {
	if (typeof data === "string") {
		return binary ? new TextEncoder().encode(data).buffer : data;
	}
	if (data instanceof ArrayBuffer) return data;
	return new Uint8Array(data.buffer, data.byteOffset, data.byteLength).slice()
		.buffer;
}

interface ServerOptions {
	readonly port?: number;
	readonly server?: HttpServer;
	readonly noServer?: boolean;
	/** Only accept upgrades for this path. */
	readonly path?: string;
}

export function wsModule(network: Network) {
	class WebSocketServer extends EventEmitter {
		readonly clients = new Set<WebSocket>();
		readonly options: ServerOptions;
		#listening: { readonly port: number; close(): void } | undefined;

		constructor(options: ServerOptions, callback?: () => void) {
			super();
			this.options = options;
			if (options.server) {
				options.server.on(
					"upgrade",
					(request: IncomingMessage, socket: UpgradeSocket) => {
						if (this.#accepts(request)) this.#upgrade(request, socket.incoming);
					},
				);
			} else if (!options.noServer) {
				if (options.port === undefined) {
					throw new TypeError(
						"WebSocketServer needs one of `port`, `server`, or `noServer`",
					);
				}
				this.#listening = network.listen(options.port, (incoming) => {
					const request = new IncomingMessage(incoming.request);
					if (incoming.kind === "http") {
						void incoming.respond(
							new Response("Upgrade Required", { status: 426 }),
						);
					} else if (this.#accepts(request)) {
						this.#upgrade(request, incoming);
					} else {
						incoming.refuse(400);
					}
				});
				queueMicrotask(() => {
					this.emit("listening");
					callback?.();
				});
			}
		}

		/** Completes an upgrade from a `node:http` server's `upgrade` event. */
		handleUpgrade(
			request: IncomingMessage,
			socket: UpgradeSocket,
			_head: Uint8Array,
			callback: (socket: WebSocket, request: IncomingMessage) => void,
		): void {
			callback(this.#open(socket.incoming), request);
		}

		shouldHandle(request: IncomingMessage): boolean {
			return this.#accepts(request);
		}

		address(): { address: string; family: string; port: number } | null {
			if (this.#listening) {
				return {
					address: "127.0.0.1",
					family: "IPv4",
					port: this.#listening.port,
				};
			}
			return this.options.server?.address() ?? null;
		}

		close(callback?: () => void): void {
			this.#listening?.close();
			for (const client of this.clients) client.terminate();
			queueMicrotask(() => {
				this.emit("close");
				callback?.();
			});
		}

		#accepts(request: IncomingMessage): boolean {
			return this.options.path === undefined ||
				request.url.split("?")[0] === this.options.path;
		}

		#upgrade(request: IncomingMessage, incoming: UpgradeIncoming): void {
			this.emit("connection", this.#open(incoming), request);
		}

		#open(incoming: UpgradeIncoming): WebSocket {
			const protocol =
				incoming.request.headers.get("sec-websocket-protocol")?.split(",")[0]
					?.trim() ?? "";
			const socket = new WebSocket(incoming.accept(protocol), protocol);
			this.clients.add(socket);
			socket.on("close", () => this.clients.delete(socket));
			return socket;
		}
	}

	return {
		WebSocketServer,
		Server: WebSocketServer,
		WebSocket,
		default: WebSocket,
	};
}
