import type { Network, UpgradeIncoming } from "../network.ts";
import { Buffer, bufferOf } from "./buffer.ts";
import { EventEmitter } from "./events.ts";

/**
 * `node:http`'s server on the sandbox network: `createServer(handler)`,
 * `listen(port)`, requests that stream their body as `data` and `end`
 * events (or `for await`), responses with `writeHead`, `setHeader`,
 * `write`, and `end`, and the `upgrade` event `ws` attaches to.
 */

export class IncomingMessage extends EventEmitter {
	readonly method: string;
	/** The path and query, as Node gives it. */
	readonly url: string;
	readonly headers: Record<string, string>;
	readonly httpVersion = "1.1";
	readonly #body: Promise<ArrayBuffer>;

	constructor(request: Request) {
		super();
		const url = new URL(request.url);
		this.method = request.method;
		this.url = url.pathname + url.search;
		this.headers = Object.fromEntries(request.headers);
		this.#body = request.arrayBuffer();
	}

	/** Emits the body once the handler has had a chance to listen. */
	stream(): void {
		void this.#body.then((body) => {
			if (body.byteLength > 0) this.emit("data", Buffer.from(body));
			this.emit("end");
		});
	}

	async *[Symbol.asyncIterator](): AsyncGenerator<Uint8Array> {
		const body = await this.#body;
		if (body.byteLength > 0) yield Buffer.from(body);
	}
}

export class ServerResponse extends EventEmitter {
	statusCode = 200;
	statusMessage = "";
	readonly #headers = new Headers();
	readonly #chunks: Uint8Array[] = [];
	readonly #finish: (response: Response) => void;
	#ended = false;

	constructor(finish: (response: Response) => void) {
		super();
		this.#finish = finish;
	}

	get headersSent(): boolean {
		return this.#ended;
	}

	get writableEnded(): boolean {
		return this.#ended;
	}

	setHeader(name: string, value: string | number | readonly string[]): this {
		this.#headers.delete(name);
		for (const item of Array.isArray(value) ? value : [value]) {
			this.#headers.append(name, String(item));
		}
		return this;
	}

	getHeader(name: string): string | undefined {
		return this.#headers.get(name) ?? undefined;
	}

	removeHeader(name: string): void {
		this.#headers.delete(name);
	}

	writeHead(
		status: number,
		message?: string | Record<string, string | number>,
		headers?: Record<string, string | number>,
	): this {
		this.statusCode = status;
		const fields = typeof message === "string" ? headers : message;
		if (typeof message === "string") this.statusMessage = message;
		for (const [name, value] of Object.entries(fields ?? {})) {
			this.setHeader(name, value);
		}
		return this;
	}

	write(chunk: string | Uint8Array): boolean {
		this.#chunks.push(bufferOf(chunk));
		return true;
	}

	end(chunk?: string | Uint8Array | (() => void), callback?: () => void): this {
		if (this.#ended) return this;
		if (typeof chunk === "function") callback = chunk;
		else if (chunk !== undefined) this.write(chunk);
		this.#ended = true;
		const body = Buffer.concat(this.#chunks);
		this.#finish(
			new Response(body.byteLength > 0 ? body : null, {
				status: this.statusCode,
				statusText: this.statusMessage,
				headers: this.#headers,
			}),
		);
		this.emit("finish");
		callback?.();
		return this;
	}
}

/** What an `upgrade` listener receives as its socket: the pending handshake. */
export class UpgradeSocket {
	/** @internal */
	readonly incoming: UpgradeIncoming;

	constructor(incoming: UpgradeIncoming) {
		this.incoming = incoming;
	}

	destroy(): void {
		this.incoming.refuse(400);
	}

	end(): void {
		this.destroy();
	}

	write(): boolean {
		return false;
	}
}

type RequestListener = (
	request: IncomingMessage,
	response: ServerResponse,
) => void;

export function httpModule(network: Network) {
	class Server extends EventEmitter {
		#listening: { readonly port: number; close(): void } | undefined;

		constructor(handler?: RequestListener) {
			super();
			if (handler) this.on("request", handler);
		}

		get listening(): boolean {
			return this.#listening !== undefined;
		}

		listen(...args: unknown[]): this {
			const port = typeof args[0] === "number"
				? args[0]
				: Number((args[0] as { port?: number } | undefined)?.port ?? 0);
			const callback = args.find((arg) => typeof arg === "function") as
				| (() => void)
				| undefined;
			this.#listening = network.listen(port, (incoming) => {
				const request = new IncomingMessage(incoming.request);
				if (incoming.kind === "websocket") {
					if (this.listenerCount("upgrade") === 0) incoming.refuse(426);
					else {this.emit(
							"upgrade",
							request,
							new UpgradeSocket(incoming),
							Buffer.alloc(0),
						);}
					return;
				}
				const response = new ServerResponse((answer) =>
					void incoming.respond(answer)
				);
				this.emit("request", request, response);
				request.stream();
			});
			queueMicrotask(() => {
				this.emit("listening");
				callback?.();
			});
			return this;
		}

		address(): { address: string; family: string; port: number } | null {
			return this.#listening
				? { address: "127.0.0.1", family: "IPv4", port: this.#listening.port }
				: null;
		}

		close(callback?: () => void): this {
			this.#listening?.close();
			this.#listening = undefined;
			queueMicrotask(() => {
				this.emit("close");
				callback?.();
			});
			return this;
		}
	}

	const createServer = (handler?: RequestListener) => new Server(handler);
	const STATUS_CODES: Record<number, string> = {
		200: "OK",
		201: "Created",
		204: "No Content",
		400: "Bad Request",
		404: "Not Found",
		405: "Method Not Allowed",
		426: "Upgrade Required",
		500: "Internal Server Error",
	};
	return {
		createServer,
		Server,
		IncomingMessage,
		ServerResponse,
		STATUS_CODES,
		default: {
			createServer,
			Server,
			IncomingMessage,
			ServerResponse,
			STATUS_CODES,
		},
	};
}

export type HttpModule = ReturnType<typeof httpModule>;
export type HttpServer = InstanceType<HttpModule["Server"]>;
