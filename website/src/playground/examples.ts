import collaborationClient from "./examples/collaboration/client.ts?raw";
import collaborationHtml from "./examples/collaboration/index.html?raw";
import collaborationServer from "./examples/collaboration/server.ts?raw";
import editorClient from "./examples/editor/client.ts?raw";
import editorHtml from "./examples/editor/index.html?raw";
import extensionsServer from "./examples/extensions/server.ts?raw";
import nodeClient from "./examples/node/client.ts?raw";
import nodeHtml from "./examples/node/index.html?raw";
import nodeServer from "./examples/node/server.ts?raw";

/**
 * The playground's starting points. Each runs server code, browser code,
 * or both; the files live in `examples/` so they read as code.
 */
export interface PlaygroundExample {
	readonly id: string;
	readonly title: string;
	/** One sentence on what it shows. */
	readonly summary: string;
	readonly server?: string;
	readonly client?: string;
	readonly html?: string;
	/** How many browser peers it starts with. */
	readonly peers: number;
}

export const EXAMPLES: readonly PlaygroundExample[] = [
	{
		id: "editor",
		title: "Editor",
		summary:
			"The browser editor, with a toolbar built from the extensions you switch on.",
		client: editorClient,
		html: editorHtml,
		peers: 1,
	},
	{
		id: "extensions",
		title: "Every extension",
		summary:
			"One document using all 49 extensions, made on the server realm, in an editor you can type in.",
		server: extensionsServer,
		peers: 0,
	},
	{
		id: "collaboration",
		title: "Collaboration",
		summary:
			"A sync server running in this tab, and editors that collaborate through it.",
		server: collaborationServer,
		client: collaborationClient,
		html: collaborationHtml,
		peers: 2,
	},
	{
		id: "node",
		title: "Node server",
		summary:
			"A node:http server that converts Markdown, called with fetch from the page.",
		server: nodeServer,
		client: nodeClient,
		html: nodeHtml,
		peers: 1,
	},
];

export const DEFAULT_EXAMPLE = "editor";
