import type {
	ClientMessage,
	ConnectRequest,
	Dial,
	Frame,
	ServerMessage,
} from "./protocol.ts";

/**
 * The server realm's `localhost`: servers listen on a port, and each dial
 * from a realm arrives as an {@link Incoming} request on its own
 * `MessagePort`. Deno's, Node's, and `ws`'s server APIs are all built on
 * this one listener shape.
 */

export interface HttpIncoming {
	readonly kind: "http";
	readonly request: Request;
	respond(response: Response): Promise<void>;
}

export interface UpgradeIncoming {
	readonly kind: "websocket";
	/** The handshake, as an `http:` request with `upgrade: websocket`. */
	readonly request: Request;
	/** Completes the handshake. */
	accept(protocol?: string): Connection;
	/** Turns the handshake down, as a server answering with `status`. */
	refuse(status: number): void;
}

export type Incoming = HttpIncoming | UpgradeIncoming;
export type Listener = (incoming: Incoming) => void;

/** The server end of a WebSocket. */
export class Connection {
	readonly #port: MessagePort;
	#open = true;
	onmessage: (frame: Frame) => void = () => {};
	onclose: (code: number, reason: string) => void = () => {};

	constructor(port: MessagePort, protocol: string) {
		this.#port = port;
		port.onmessage = (event: MessageEvent<ClientMessage>) => {
			const message = event.data;
			if (message.type === "message") this.onmessage(message.data);
			else this.#closed(message.code, message.reason);
		};
		post(port, { type: "open", protocol });
	}

	get open(): boolean {
		return this.#open;
	}

	send(frame: Frame): void {
		if (!this.#open) return;
		const data = typeof frame === "string" ? frame : frame.slice(0);
		post(
			this.#port,
			{ type: "message", data },
			typeof data === "string" ? [] : [data],
		);
	}

	close(code = 1000, reason = ""): void {
		if (!this.#open) return;
		post(this.#port, { type: "close", code, reason });
		this.#closed(code, reason);
	}

	#closed(code: number, reason: string): void {
		if (!this.#open) return;
		this.#open = false;
		this.#port.close();
		this.onclose(code, reason);
	}
}

function post(
	target: MessagePort,
	message: ServerMessage,
	transfer: Transferable[] = [],
): void {
	target.postMessage(message, transfer);
}

/** An error with Node's `code`, as Node and Deno throw for a taken port. */
export class AddressInUse extends Error {
	readonly code = "EADDRINUSE";
	constructor(portNumber: number) {
		super(`Port ${portNumber} is already in use in the sandbox`);
		this.name = "AddrInUse";
	}
}

const FIRST_EPHEMERAL_PORT = 49152;

export class Network {
	readonly #listeners = new Map<number, Listener>();
	#nextEphemeral = FIRST_EPHEMERAL_PORT;
	/** Called with each port a server starts listening on. */
	onlisten: (port: number) => void = () => {};

	/** Listens on `port` (`0` picks a free one) and returns the port and a way to stop. */
	listen(
		requested: number,
		listener: Listener,
	): { readonly port: number; close(): void } {
		const portNumber = requested === 0 ? this.#ephemeral() : requested;
		if (this.#listeners.has(portNumber)) throw new AddressInUse(portNumber);
		this.#listeners.set(portNumber, listener);
		this.onlisten(portNumber);
		return {
			port: portNumber,
			close: () => {
				if (this.#listeners.get(portNumber) === listener) {
					this.#listeners.delete(portNumber);
				}
			},
		};
	}

	/** Hands a dialled connection to whatever listens on its port. */
	accept(dial: Dial, channel: MessagePort): void {
		const listener = this.#listeners.get(dial.port);
		if (listener === undefined) {
			post(channel, { type: "refused" });
			channel.close();
			return;
		}
		listener(incoming(dial.request, channel));
	}

	#ephemeral(): number {
		while (this.#listeners.has(this.#nextEphemeral)) this.#nextEphemeral++;
		return this.#nextEphemeral++;
	}
}

function incoming(request: ConnectRequest, channel: MessagePort): Incoming {
	if (request.kind === "http") {
		const hasBody = request.body !== null && request.method !== "GET" &&
			request.method !== "HEAD";
		return {
			kind: "http",
			request: new Request(request.url, {
				method: request.method,
				headers: request.headers.map((
					[name, value],
				): [string, string] => [name, value]),
				body: hasBody ? request.body : null,
			}),
			async respond(response) {
				const body = await response.arrayBuffer();
				post(channel, {
					type: "response",
					status: response.status,
					statusText: response.statusText,
					headers: [...response.headers],
					body,
				}, [body]);
				channel.close();
			},
		};
	}

	const url = new URL(request.url);
	url.protocol = url.protocol === "wss:" ? "https:" : "http:";
	const headers = new Headers({
		upgrade: "websocket",
		connection: "Upgrade",
		"sec-websocket-version": "13",
	});
	if (request.protocols.length > 0) {
		headers.set("sec-websocket-protocol", request.protocols.join(", "));
	}
	let settled = false;
	return {
		kind: "websocket",
		request: new Request(url, { headers }),
		accept(protocol = "") {
			if (settled) {
				throw new Error("The WebSocket handshake has already been answered");
			}
			settled = true;
			return new Connection(channel, protocol);
		},
		refuse(status) {
			if (settled) return;
			settled = true;
			post(channel, {
				type: "response",
				status,
				statusText: "",
				headers: [],
				body: new ArrayBuffer(0),
			});
			channel.close();
		},
	};
}
