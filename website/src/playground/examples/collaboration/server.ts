import { createEngine, type Doc, type StarterKit } from "@canvist/headless";

// The sync server from the collaboration guide, with presence added: who is
// in each room and where their cursor is. It runs in this tab; the editors
// on the right connect to it.
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
			"# New document\n\nType in either editor, and watch the other one's cursor.",
		);
		doc.share(1);
		found = { doc, sockets: new Set() };
		rooms.set(name, found);
	}
	return found;
}

function sync(name: string, socket: WebSocket) {
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
}

// Presence isn't part of the document, so it has its own socket per peer.
// Each peer sends JSON (its id, name, colour, and cursor) whenever that
// changes; the server keeps the latest from each and passes it on.
const presence = new Map<string, Map<WebSocket, string | undefined>>();

function relayPresence(name: string, socket: WebSocket) {
	const peers = presence.get(name) ??
		new Map<WebSocket, string | undefined>();
	presence.set(name, peers);
	const others = () => [...peers.keys()].filter((peer) => peer !== socket);

	socket.onopen = () => {
		// A newcomer sees everyone who is already here.
		for (const state of peers.values()) if (state) socket.send(state);
		peers.set(socket, undefined);
	};
	socket.onmessage = (event) => {
		peers.set(socket, event.data);
		for (const other of others()) other.send(event.data);
	};
	socket.onclose = () => {
		const last = peers.get(socket);
		peers.delete(socket);
		if (last === undefined) return;
		const left = JSON.stringify({ id: JSON.parse(last).id, left: true });
		for (const other of others()) other.send(left);
	};
}

Deno.serve({ port: 8787 }, (request) => {
	const { socket, response } = Deno.upgradeWebSocket(request);
	const path = new URL(request.url).pathname;
	if (path.startsWith("/presence/")) {
		relayPresence(path.slice("/presence/".length), socket);
	} else {
		sync(path.slice(1), socket);
	}
	return response;
});
