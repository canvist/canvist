import { assert, assertEquals, assertRejects } from "@std/assert";
import { liveRealm } from "../../src/lib/live-samples.ts";
import {
	type Dialer,
	sandboxFetch,
	sandboxWebSocket,
} from "../../src/sandbox/client.ts";
import { compile, errorLine, evaluate } from "../../src/sandbox/compile.ts";
import { denoNamespace } from "../../src/sandbox/deno.ts";
import { expectedOutput } from "../../src/sandbox/expected.ts";
import { inspect } from "../../src/sandbox/inspect.ts";
import {
	BROWSER_MODULES,
	browserModule,
	importsOf,
	realmOf,
	serverModule,
} from "../../src/sandbox/modules.ts";
import { Network } from "../../src/sandbox/network.ts";
import { httpModule } from "../../src/sandbox/node/http.ts";
import { wsModule } from "../../src/sandbox/node/ws.ts";

const EXAMPLES = new URL("../../src/playground/examples/", import.meta.url);

/** A dialer straight into `network`, as the server realm dials itself. */
function dialer(network: Network): Dialer {
	return (port, request) => {
		const channel = new MessageChannel();
		network.accept({ type: "dial", port, request }, channel.port2);
		return channel.port1;
	};
}

function nextMessage(socket: WebSocket): Promise<unknown> {
	return new Promise((resolve) =>
		socket.addEventListener("message", (event) => resolve(event.data), {
			once: true,
		})
	);
}

function opened(socket: WebSocket): Promise<void> {
	return new Promise((resolve) =>
		socket.addEventListener("open", () => resolve(), { once: true })
	);
}

Deno.test("console output reads like Deno's", () => {
	assertEquals(
		inspect({ rejectedBlocks: 0, waiting: false }),
		"{ rejectedBlocks: 0, waiting: false }",
	);
	assertEquals(inspect(["heading", "paragraph"]), '[ "heading", "paragraph" ]');
	assertEquals(
		inspect([{ height: 200, strokes: 1 }]),
		"[ { height: 200, strokes: 1 } ]",
	);
	assertEquals(inspect(new Uint8Array([1, 2])), "Uint8Array(2) [ 1, 2 ]");
	assertEquals(inspect(new Map([["a", 1]])), 'Map(1) { "a" => 1 }');
	assertEquals(inspect({ "kebab-key": [] }), '{ "kebab-key": [] }');
	const circular: Record<string, unknown> = {};
	circular["self"] = circular;
	assertEquals(inspect(circular), "{ self: [Circular] }");
	assertEquals(
		inspect({
			first: "a fairly long string value",
			second: "another fairly long string value",
		}),
		'{\n  first: "a fairly long string value",\n  second: "another fairly long string value"\n}',
	);
});

Deno.test("code runs in the realm whose modules it imports", () => {
	assertEquals(
		importsOf(
			`import { a } from "x";\nimport "y";\nconst z = await import("x");`,
		),
		["x", "y"],
	);
	assertEquals(
		realmOf(`import { createEngine } from "@canvist/headless";`),
		"server",
	);
	assertEquals(realmOf(`import { createServer } from "http";`), "server");
	assertEquals(
		realmOf(
			`import { createEditor } from "@canvist/web";\nimport { bold } from "@canvist/headless";`,
		),
		"browser",
	);
	assertEquals(
		realmOf(`import { readFile } from "node:fs/promises";`),
		undefined,
	);
	assertEquals(
		realmOf(
			`import * as Y from "yjs";\nimport { createEditor } from "@canvist/web";`,
		),
		undefined,
	);
	assertEquals(serverModule("events"), "node:events");
	assertEquals(browserModule("@canvist/web/collab"), "@canvist/web/collab");
});

Deno.test("a docs sample is live when it runs, or says `live`", () => {
	const headless = `import { createEngine } from "@canvist/headless";`;
	const web = `import { createEditor } from "@canvist/web";`;
	assertEquals(liveRealm("ts", "", headless), "server");
	assertEquals(liveRealm("ts", "no_run", headless), undefined);
	assertEquals(liveRealm("ts", "no_run live", web), "browser");
	assertEquals(liveRealm("tsx", "", headless), undefined);
	assertEquals(
		liveRealm("ts", "", `import { readFile } from "node:fs/promises";`),
		undefined,
	);
});

Deno.test("expected output is the comments after each console.log", () => {
	assertEquals(
		expectedOutput(
			`console.log(a);\n// one\n// two\nfoo();\nconsole.log(b);\n// three`,
		),
		["one", "two", "three"],
	);
	assertEquals(expectedOutput(`console.log(a); // inline`), undefined);
});

Deno.test("compiled code keeps top-level await, requires its imports, and reports lines as written", async () => {
	const source =
		`import { double } from "math";\nconst value: number = await Promise.resolve(21);\nconsole.log(double(value));\nthrow new Error("on line four");\n`;
	const compiled = compile(source);
	assertEquals(compiled.requires, ["math"]);
	const printed: unknown[] = [];
	const error = await assertRejects(() =>
		evaluate(compiled, "sample.ts", {
			require: () => ({ __esModule: true, double: (n: number) => n * 2 }),
			console: {
				...console,
				log: (...args: unknown[]) => printed.push(...args),
			},
		})
	);
	assertEquals(printed, [42]);
	assertEquals(errorLine(error, "sample.ts"), 4);
});

Deno.test("fetch to localhost reaches a node:http server on the sandbox network", async () => {
	const network = new Network();
	const { createServer } = httpModule(network);
	const server = createServer(async (request, response) => {
		let body = "";
		for await (const chunk of request) body += chunk;
		response.writeHead(201, { "content-type": "text/plain" }).end(
			`${request.method} ${request.url} ${body}`,
		);
	});
	server.listen(3000);
	const sandboxed = sandboxFetch(
		dialer(network),
		() => Promise.reject(new Error("left the sandbox")),
	);

	const response = await sandboxed("http://localhost:3000/convert?to=html", {
		method: "POST",
		body: "# Hi",
	});
	assertEquals(response.status, 201);
	assertEquals(response.headers.get("content-type"), "text/plain");
	assertEquals(await response.text(), "POST /convert?to=html # Hi");
	await assertRejects(
		() => sandboxed("http://localhost:4000/"),
		TypeError,
		"nothing is listening on port 4000",
	);
	server.close();
});

Deno.test("WebSockets to localhost reach a `ws` server, and messages arrive as Buffers", async () => {
	const network = new Network();
	const { WebSocketServer } = wsModule(network);
	const server = new WebSocketServer({ port: 8080 });
	server.on(
		"connection",
		(
			socket: {
				on(event: string, listener: (...args: never[]) => void): void;
				send(data: string): void;
			},
			request: { url: string },
		) => {
			socket.on(
				"message",
				(data: Uint8Array & { toString(): string }, isBinary: boolean) => {
					socket.send(
						`${request.url} got ${data.toString()} (binary: ${isBinary})`,
					);
				},
			);
		},
	);
	const Socket = sandboxWebSocket(dialer(network), undefined);
	const socket = new Socket("ws://localhost:8080/room");
	await opened(socket);
	const reply = nextMessage(socket);
	socket.send("hello");
	assertEquals(await reply, "/room got hello (binary: false)");
	socket.close();
	server.close();
});

Deno.test("Deno.serve answers HTTP and upgrades WebSockets, as the collaboration guide's server does", async () => {
	const network = new Network();
	const errors: unknown[] = [];
	const deno = denoNamespace(network, {}, (error) => errors.push(error));
	const server = deno.serve({ port: 8787, onListen() {} }, (request) => {
		if (request.headers.get("upgrade") !== "websocket") {
			return new Response(`plain ${new URL(request.url).pathname}`);
		}
		const { socket, response } = deno.upgradeWebSocket(request);
		socket.binaryType = "arraybuffer";
		socket.onmessage = (event) =>
			socket.send(new Uint8Array(event.data as ArrayBuffer).reverse());
		return response;
	});
	const dial = dialer(network);

	assertEquals(
		await (await sandboxFetch(dial, fetch)("http://localhost:8787/rooms"))
			.text(),
		"plain /rooms",
	);

	const Socket = sandboxWebSocket(dial, undefined);
	const socket = new Socket("ws://localhost:8787/notes");
	socket.binaryType = "arraybuffer";
	await opened(socket);
	const reply = nextMessage(socket);
	socket.send(new Uint8Array([1, 2, 3]));
	assertEquals([...new Uint8Array(await reply as ArrayBuffer)], [3, 2, 1]);
	socket.close();
	await server.shutdown();
	assertEquals(errors, []);
});

Deno.test("a socket to a port nobody listens on errors and closes, as a refused connection does", async () => {
	const Socket = sandboxWebSocket(dialer(new Network()), undefined);
	const socket = new Socket("ws://localhost:9999");
	const closed = await new Promise<CloseEvent>((resolve) =>
		socket.addEventListener("close", (event) => resolve(event as CloseEvent))
	);
	assertEquals(closed.code, 1006);
	assertEquals(socket.readyState, socket.CLOSED);
});

Deno.test("every playground example compiles and imports only what its realm has", async () => {
	for await (const entry of Deno.readDir(EXAMPLES)) {
		const directory = new URL(`${entry.name}/`, EXAMPLES);
		for (const file of ["server.ts", "client.ts"]) {
			let source: string;
			try {
				source = await Deno.readTextFile(new URL(file, directory));
			} catch {
				continue;
			}
			const { requires } = compile(source);
			const known = file === "server.ts"
				? requires.every((name) => serverModule(name))
				: requires.every((name) =>
					BROWSER_MODULES.some((module) => module === name)
				);
			assert(known, `${entry.name}/${file} imports ${requires.join(", ")}`);
		}
	}
});
