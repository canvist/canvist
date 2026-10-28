import { type Dialer, sandboxFetch, sandboxWebSocket } from "./client.ts";
import { denoNamespace } from "./deno.ts";
import { DocumentTracker } from "./documents.ts";
import { SERVER_MODULES, type ServerModule, serverModule } from "./modules.ts";
import { Network } from "./network.ts";
import { Buffer } from "./node/buffer.ts";
import { EventEmitter } from "./node/events.ts";
import { httpModule } from "./node/http.ts";
import { wsModule } from "./node/ws.ts";
import type { RealmEvent, WorkerCommand } from "./protocol.ts";
import { capturedConsole, runError, runFile } from "./realm.ts";

/**
 * The server realm: a worker that runs one file of server code, the way
 * `node main.ts` or `deno run main.ts` would, on a network of its own. The
 * page dials into it for every `localhost` connection a browser realm opens.
 *
 * Each run gets a fresh worker, so a server from the last run never
 * answers for this one, and stopping a run is terminating the worker.
 */

/** The worker's global scope: what this file uses of it, typed without the webworker lib. */
const scope = globalThis as unknown as {
	console: Console;
	WebSocket: typeof WebSocket;
	fetch: typeof fetch;
	postMessage(message: RealmEvent): void;
	close(): void;
	addEventListener: typeof addEventListener;
};
const emit = (event: RealmEvent) => scope.postMessage(event);
let file = "server.ts";

const network = new Network();
network.onlisten = (port) => emit({ type: "listening", port });
const tracker = new DocumentTracker((documents) =>
	emit({ type: "documents", documents })
);
const report = (error: unknown) =>
	emit({ type: "uncaught", error: runError(error, file) });

// Connections the server realm opens to itself go straight to its network.
const dial: Dialer = (port, request) => {
	const channel = new MessageChannel();
	network.accept({ type: "dial", port, request }, channel.port2);
	return channel.port1;
};

const env: Record<string, string> = {};
const console = capturedConsole(emit, scope.console);

class Process extends EventEmitter {
	readonly env = env;
	readonly platform = "browser";
	readonly version = "v24.0.0-sandbox";
	readonly versions = { sandbox: "1" };
	get argv(): string[] {
		return ["node", file];
	}
	cwd(): string {
		return "/";
	}
	nextTick(callback: (...args: unknown[]) => void, ...args: unknown[]): void {
		queueMicrotask(() => callback(...args));
	}
	exit(code = 0): never {
		emit({
			type: "log",
			level: "info",
			text: `Process exited with code ${code}`,
		});
		scope.close();
		throw new Error("process.exit");
	}
}

Object.assign(scope, {
	console,
	Buffer,
	process: new Process(),
	Deno: denoNamespace(network, env, report),
	WebSocket: sandboxWebSocket(dial, scope.WebSocket),
	fetch: sandboxFetch(dial, scope.fetch.bind(scope)),
});

/** `@canvist/headless` and `/full` share their classes; documents from either are previewed. */
const watched = <M extends Parameters<DocumentTracker["watch"]>[0]>(
	module: M,
): M => {
	tracker.watch(module);
	return module;
};

const LOADERS: Record<ServerModule, () => Promise<unknown>> = {
	"@canvist/headless": () => import("@canvist/headless").then(watched),
	"@canvist/headless/full": () =>
		import("@canvist/headless/full").then(watched),
	"@canvist/headless/ssr": () => import("@canvist/headless/ssr"),
	ws: () => Promise.resolve(wsModule(network)),
	"node:http": () => Promise.resolve(httpModule(network)),
	"node:events": () => Promise.resolve({ EventEmitter, default: EventEmitter }),
};

const loaded = new Map<ServerModule, Promise<unknown>>();

function load(specifier: string): Promise<unknown> | undefined {
	const name = serverModule(specifier);
	if (name === undefined) return undefined;
	let loading = loaded.get(name);
	if (loading === undefined) {
		loading = LOADERS[name]();
		loaded.set(name, loading);
	}
	return loading;
}

scope.addEventListener("error", (event) => {
	event.preventDefault();
	report(event.error ?? event.message);
});
scope.addEventListener("unhandledrejection", (event) => {
	event.preventDefault();
	report(event.reason);
});

scope.addEventListener("message", (event: MessageEvent<WorkerCommand>) => {
	const command = event.data;
	if (command.type === "dial") {
		const [channel] = event.ports;
		if (channel) network.accept(command, channel);
		return;
	}
	file = command.file;
	runFile({
		file,
		source: command.source,
		console,
		load,
		available: SERVER_MODULES,
	}).then(
		() => {
			tracker.flush();
			emit({ type: "settled" });
		},
		(error: unknown) => {
			tracker.flush();
			emit({ type: "settled", error: runError(error, file) });
		},
	);
});
