import { createEngine, type Doc, type StarterKit } from "@canvist/headless";

// The sync server from the collaboration guide, unchanged but for its log
// lines. It runs in this tab; the editors on the right connect to it.
const engine = await createEngine();
const rooms = new Map<
	string,
	{ doc: Doc<StarterKit>; sockets: Set<WebSocket> }
>();

function room(name: string) {
	let found = rooms.get(name);
	if (found === undefined) {
		const doc = engine.parse(
			"markdown",
			"# New document\n\nType in either editor.",
		);
		doc.share(1);
		found = { doc, sockets: new Set() };
		rooms.set(name, found);
	}
	return found;
}

Deno.serve({ port: 8787 }, (request) => {
	const { socket, response } = Deno.upgradeWebSocket(request);
	const name = new URL(request.url).pathname.slice(1);
	const { doc, sockets } = room(name);
	socket.binaryType = "arraybuffer";

	socket.onopen = () => {
		sockets.add(socket);
		console.log(`A peer joined "${name}" (${sockets.size} connected)`);
		socket.send(doc.syncStart());
	};
	socket.onmessage = (event) => {
		const { reply, broadcast } = doc.receiveSync(new Uint8Array(event.data));
		reply.forEach((message) => socket.send(message));
		for (const other of sockets) {
			if (other !== socket) broadcast.forEach((message) => other.send(message));
		}
	};
	socket.onclose = () => {
		sockets.delete(socket);
		console.log(`A peer left "${name}" (${sockets.size} connected)`);
	};
	return response;
});
