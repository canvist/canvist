import type { Position } from "@canvist/web";
import { connect, createEditor } from "@canvist/web/collab";

const element = document.querySelector<HTMLElement>("#editor")!;
const status = document.querySelector<HTMLElement>("#status")!;
const cursors = document.querySelector<HTMLElement>("#cursors")!;

// Who this peer is. The playground puts each peer's name and colour in its
// URL; an app would use the signed-in person's.
const params = new URLSearchParams(location.search);
const me = {
	name: params.get("name") ?? "Guest",
	color: params.get("color") ?? "#3B2CF5",
};

// Each peer is its own editor in its own frame. `localhost:8787` is the
// server on the left, running in this tab.
const editor = await createEditor(element, {
	placeholder: "Write together…",
	label: "Shared notes",
});
const connection = connect(editor, {
	url: "ws://localhost:8787",
	room: "notes",
});

connection.subscribe(() => {
	status.textContent = connection.synced
		? `Synced as ${me.name}`
		: `${connection.status[0]?.toUpperCase()}${connection.status.slice(1)}…`;
});
await connection.whenSynced();

// ---------- Presence: everyone else's cursor ----------

interface Presence {
	readonly id: number;
	readonly name: string;
	readonly color: string;
	/** Where their caret is, or `null` while they aren't typing here. */
	readonly head: Position | null;
}

const presence = new WebSocket("ws://localhost:8787/presence/notes");
const others = new Map<number, { state: Presence; cursor: HTMLElement }>();
let sent = "";

// Tell the others where this peer's caret is, whenever it moves.
function share() {
	const head = editor.focused ? editor.selection?.head ?? null : null;
	const state: Presence = { id: connection.clientId, ...me, head };
	const message = JSON.stringify(state);
	if (message === sent || presence.readyState !== WebSocket.OPEN) return;
	presence.send(message);
	sent = message;
}

presence.onopen = share;
presence.onmessage = (event) => {
	const message = JSON.parse(event.data) as Presence | {
		id: number;
		left: true;
	};
	if ("left" in message) {
		others.get(message.id)?.cursor.remove();
		others.delete(message.id);
	} else {
		let other = others.get(message.id);
		if (other === undefined) {
			const cursor = document.createElement("div");
			cursor.className = "cursor";
			cursor.append(document.createElement("span"));
			cursors.append(cursor);
			other = { state: message, cursor };
			others.set(message.id, other);
		}
		other.state = message;
		other.cursor.style.setProperty("--color", message.color);
		other.cursor.firstElementChild!.textContent = message.name;
	}
	draw();
};

// Draw each caret where it is in this editor's layout of the document.
function draw() {
	const box = cursors.getBoundingClientRect();
	for (const { state, cursor } of others.values()) {
		const rect = state.head && editor.caretRectAt(state.head);
		cursor.hidden = !rect;
		if (!rect) continue;
		const { left, top, height } = editor.toClientRect(rect);
		cursor.style.translate = `${left - box.left}px ${top - box.top}px`;
		cursor.style.height = `${height}px`;
	}
}

// Edits here, edits from others, and moving the caret all come through
// `subscribe`, so the cursors and what this peer shares stay current.
editor.subscribe(() => {
	share();
	draw();
});
new ResizeObserver(draw).observe(element);
