import { connect, createEditor } from "@canvist/web/collab";

const element = document.querySelector<HTMLElement>("#editor")!;
const status = document.querySelector<HTMLElement>("#status")!;

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
		? "Synced"
		: `${connection.status[0]?.toUpperCase()}${connection.status.slice(1)}…`;
});
await connection.whenSynced();
