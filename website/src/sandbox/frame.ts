import { type Dialer, sandboxFetch, sandboxWebSocket } from "./client.ts";
import {
	BROWSER_MODULES,
	type BrowserModule,
	browserModule,
} from "./modules.ts";
import type { FrameCommand, FrameMessage, RealmEvent } from "./protocol.ts";
import { capturedConsole, runError, runFile } from "./realm.ts";

/**
 * A browser realm: the page at `/sandbox/frame`, which runs one file of
 * browser code against the markup it's given. Its `localhost` connections
 * are dialled through the page that embeds it, to that page's server realm.
 *
 * It only takes code from the page that embeds it, which is always this
 * site's own: the frame is never pointed at code from anywhere else.
 */

const parentWindow = window.parent;
const post = (message: FrameMessage, transfer: Transferable[] = []) =>
	parentWindow.postMessage(message, location.origin, transfer);
const emit = (event: RealmEvent) => post({ type: "event", event });
let file = "client.ts";

const dial: Dialer = (port, request) => {
	const channel = new MessageChannel();
	post({ type: "dial", port, request }, [channel.port2]);
	return channel.port1;
};

const console = capturedConsole(emit, window.console);
const sockets = new Set<{ close(code?: number, reason?: string): void }>();
Object.assign(window, {
	console,
	WebSocket: sandboxWebSocket(dial, window.WebSocket, sockets),
	fetch: sandboxFetch(dial, window.fetch.bind(window)),
});

const LOADERS: Record<BrowserModule, () => Promise<unknown>> = {
	"@canvist/web": () => import("@canvist/web"),
	"@canvist/web/collab": () => import("@canvist/web/collab"),
	"@canvist/web/menus": () => import("@canvist/web/menus"),
	"@canvist/headless": () => import("@canvist/headless"),
	"@canvist/headless/full": () => import("@canvist/headless/full"),
};

function load(specifier: string): Promise<unknown> | undefined {
	const name = browserModule(specifier);
	return name === undefined ? undefined : LOADERS[name]();
}

window.addEventListener("error", (event) => {
	event.preventDefault();
	emit({
		type: "uncaught",
		error: runError(event.error ?? event.message, file),
	});
});
window.addEventListener("unhandledrejection", (event) => {
	event.preventDefault();
	emit({ type: "uncaught", error: runError(event.reason, file) });
});

new ResizeObserver(() =>
	post({ type: "resize", height: document.documentElement.scrollHeight })
).observe(document.body);

let ran = false;
window.addEventListener("message", (event: MessageEvent<FrameCommand>) => {
	// Only the embedding page, on this site, may run code here, and only once.
	if (event.source !== parentWindow || event.origin !== location.origin) return;
	const command = event.data;
	if (command.type === "close") {
		// Going away, as a closing tab does: servers hear each socket close.
		for (const socket of sockets) socket.close(1001, "The peer left");
		return;
	}
	if (ran) return;
	ran = true;
	file = command.file;
	document.documentElement.style.colorScheme = command.scheme;
	document.body.innerHTML = command.html;
	runFile({
		file,
		source: command.source,
		console,
		load,
		available: BROWSER_MODULES,
	}).then(
		() => emit({ type: "settled" }),
		(error: unknown) => emit({ type: "settled", error: runError(error, file) }),
	);
});

post({ type: "ready" });
