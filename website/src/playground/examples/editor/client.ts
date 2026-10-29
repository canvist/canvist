import {
	blockquote,
	bold,
	bulletList,
	createEditor,
	heading,
	history,
	italic,
	link,
	listItem,
} from "@canvist/web";

const element = document.querySelector<HTMLElement>("#editor")!;
const toolbar = document.querySelector<HTMLElement>("#toolbar")!;

// Switch extensions on the left: the list below changes, and so does what
// the document can hold. Strike, task lists, and code blocks are off, so
// their Markdown arrives as plain text until you switch them on. Switching
// on one beyond the StarterKit moves the import to @canvist/web/full.
const editor = await createEditor(element, {
	extensions: [
		heading,
		blockquote,
		bulletList,
		listItem,
		bold,
		italic,
		link,
		history,
	],
	content: {
		format: "markdown",
		input: [
			"# Switch extensions on and off",
			"",
			"**Bold**, *italic*, and ~~struck~~ text only look that way while their extensions are on.",
			"",
			"- A bullet list",
			"- with two items",
			"",
			"* [ ] And a task, once Task list is on",
			"",
			"> Type `## ` at the start of a line for a heading.",
			"",
			"```",
			"code, with Code block on",
			"```",
		].join("\n"),
	},
	label: "Playground editor",
});

// A button for every toggle the registered extensions bring, so the
// toolbar grows and shrinks with the list above.
const queries = new Set(
	editor.extensions.flatMap((extension) =>
		extension.queries.map((query) => query.name)
	),
);
const toggles = editor.extensions
	.flatMap((extension) => extension.commands)
	.filter((command) =>
		command.name.startsWith("toggle") && command.args.length === 0
	)
	.map((command) => {
		const label = command.name.slice("toggle".length);
		const button = document.createElement("button");
		button.type = "button";
		button.textContent = label;
		button.addEventListener("click", () => {
			editor.run(command.name);
			editor.focus();
		});
		return {
			button,
			command: command.name,
			query: queries.has(`is${label}`) ? `is${label}` : undefined,
		};
	});
toolbar.replaceChildren(...toggles.map(({ button }) => button));

function refresh() {
	for (const { button, command, query } of toggles) {
		button.disabled = !editor.can[command]();
		if (query) button.setAttribute("aria-pressed", String(editor.ask(query)));
	}
}
editor.subscribe(refresh);
refresh();
