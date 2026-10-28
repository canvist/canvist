import { ConsoleView } from "../sandbox/console-view.ts";
import {
	type CodeEditor,
	createCodeEditor,
	type Language,
} from "../sandbox/editor.ts";
import {
	allowsExtras,
	type Catalog,
	type ExtensionList,
	readExtensionList,
	writeExtensionList,
} from "../sandbox/extension-list.ts";
import { takePlaygroundHandoff } from "../sandbox/handoff.ts";
import {
	type BrowserPeer,
	type RealmSource,
	Sandbox,
} from "../sandbox/host.ts";
import { DocumentPreview } from "../sandbox/preview.ts";
import type {
	DocSnapshot,
	LogLevel,
	RealmEvent,
	RunError,
} from "../sandbox/protocol.ts";
import { EXAMPLES, type PlaygroundExample } from "./examples.ts";

/**
 * The playground page: an example's files in editors, the extension
 * switches bound to the code, and a sandbox that runs the server file in a
 * worker and the browser file in one frame per peer. Everything reruns a
 * moment after the code changes.
 *
 * Agents drive it through `window.canvistPlayground` (see
 * {@link PlaygroundApi}), and read `data-state` on the root.
 */

export type FileName = "server.ts" | "client.ts" | "index.html";

const FILES: readonly {
	readonly name: FileName;
	readonly key: "server" | "client" | "html";
	readonly language: Language;
}[] = [
	{ name: "server.ts", key: "server", language: "ts" },
	{ name: "client.ts", key: "client", language: "ts" },
	{ name: "index.html", key: "html", language: "html" },
];

/** The four presence colours from the landing page's Together room, one per peer. */
const PEER_COLORS = ["#3B2CF5", "#E5484D", "#0E9384", "#C07A00"];
const MAX_PEERS = PEER_COLORS.length;
const RERUN_DELAY_MS = 700;
/** How long browser peers wait for the server to start listening. */
const SERVER_START_MS = 4000;
/** How long `run()` waits for every realm to finish its top level. */
const RUN_TIMEOUT_MS = 20_000;
/**
 * After that, how long the realms must stay quiet before `run()` reports,
 * so what they do just after starting (peers joining, a document snapshot)
 * is in the result. It never waits longer than `QUIET_LIMIT_MS`.
 */
const QUIET_MS = 150;
const QUIET_LIMIT_MS = 1500;

export interface LogEntry {
	/** `server`, or `peer 1` and up. */
	readonly source: string;
	readonly level: LogLevel | "uncaught";
	readonly text: string;
}

export interface RunResult {
	/** Whether every realm's code ran without throwing. */
	readonly ok: boolean;
	readonly errors: readonly {
		readonly source: string;
		readonly file: FileName;
		readonly message: string;
		readonly line?: number;
	}[];
	readonly logs: readonly LogEntry[];
	/** The server's documents, most recently used first. */
	readonly documents: readonly DocSnapshot[];
	/** The ports the server listens on. */
	readonly ports: readonly number[];
}

/** What `window.canvistPlayground` offers people and agents scripting the page. */
export interface PlaygroundApi {
	readonly examples: readonly string[];
	readonly example: string;
	/** Opens an example and runs it. */
	load(example: string): Promise<RunResult>;
	/** A file's code, or `undefined` when the example has no such file. */
	read(file: FileName): string | undefined;
	/** Replaces a file's code and runs everything. */
	write(file: FileName, code: string): Promise<RunResult>;
	run(): Promise<RunResult>;
	/** The extensions the code registers, and whether the switches can change them. */
	extensions(): {
		readonly enabled: readonly string[];
		readonly editable: boolean;
		readonly note: string;
	};
	/** Rewrites the code to register exactly `names`, then runs it. */
	setExtensions(names: readonly string[]): Promise<RunResult>;
	/** Adds a browser peer running the client code. */
	addPeer(): Promise<void>;
	/** `fetch` against the server realm's `localhost`. */
	fetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response>;
	/** Everything printed since the last run started. */
	logs(): readonly LogEntry[];
	/** The server's documents as they are now, most recently used first. */
	documents(): readonly DocSnapshot[];
}

declare global {
	interface Window {
		canvistPlayground?: PlaygroundApi;
	}
}

interface OpenFile {
	readonly name: FileName;
	readonly editor: CodeEditor;
	readonly tab: HTMLButtonElement;
	readonly panel: HTMLElement;
}

interface Peer {
	readonly peer: BrowserPeer;
	readonly element: HTMLElement;
	readonly label: string;
}

export function mountPlayground(root: HTMLElement): void {
	const find = <E extends Element>(selector: string): E => {
		const found = root.querySelector<E>(selector);
		if (found === null) {
			throw new Error(`The playground is missing ${selector}`);
		}
		return found;
	};
	const catalog = JSON.parse(root.dataset["catalog"] ?? "{}") as Catalog;
	const tabs = find<HTMLElement>("[data-files]");
	const editors = find<HTMLElement>("[data-editors]");
	const status = find<HTMLElement>("[data-status]");
	const ports = find<HTMLElement>("[data-ports]");
	const peersSection = find<HTMLElement>("[data-peers-section]");
	const peersGrid = find<HTMLElement>("[data-peers]");
	const addPeerButton = find<HTMLButtonElement>("[data-add-peer]");
	const serverDocSection = find<HTMLElement>("[data-server-doc-section]");
	const extensionNote = find<HTMLElement>("[data-extensions-note]");
	const extensionCount = find<HTMLElement>("[data-extensions-count]");
	const examplePicker = find<HTMLFieldSetElement>(".pg-examples");
	const checkboxes = [
		...root.querySelectorAll<HTMLInputElement>('input[name="extension"]'),
	];

	const log = new ConsoleView(
		find("[data-console]"),
		(line) => focusLine(lastErrorFile, line),
	);
	const preview = new DocumentPreview(find("[data-server-doc]"));
	const sandbox = new Sandbox(onEvent, { fitFrames: true });

	let example: PlaygroundExample = EXAMPLES[0]!;
	let files: OpenFile[] = [];
	let peers: Peer[] = [];
	let peerCount = 0;
	let runToken = 0;
	let rerun: ReturnType<typeof setTimeout> | undefined;
	let lastErrorFile: FileName = "server.ts";
	let result = emptyResult();
	let onListening: (() => void) | undefined;
	let lastEvent = 0;

	const file = (name: FileName) => files.find((open) => open.name === name);

	// ---------- Examples and files ----------

	function load(next: PlaygroundExample): Promise<RunResult> {
		example = next;
		for (const open of files) open.editor.destroy();
		files = [];
		tabs.replaceChildren();
		editors.replaceChildren();
		for (const { name, key, language } of FILES) {
			const code = next[key];
			if (code === undefined) continue;
			const tab = document.createElement("button");
			tab.type = "button";
			tab.textContent = name;
			tab.id = `pg-file-${name.replace(".", "-")}`;
			tab.setAttribute("role", "tab");
			const panel = document.createElement("div");
			panel.id = `${tab.id}-panel`;
			panel.setAttribute("role", "tabpanel");
			panel.setAttribute("aria-labelledby", tab.id);
			tab.setAttribute("aria-controls", panel.id);
			editors.append(panel);
			tabs.append(tab);
			const editor = createCodeEditor(panel, {
				doc: code.replace(/\n$/, ""),
				language,
				label: `${name}, editable`,
				onRun: () => void run(),
				onChange: () => {
					syncExtensions();
					clearTimeout(rerun);
					rerun = setTimeout(() => void run(), RERUN_DELAY_MS);
				},
			});
			const open: OpenFile = { name, editor, tab, panel };
			files.push(open);
			tab.addEventListener("click", () => select(name));
			tab.addEventListener("keydown", (event) => {
				const step = event.key === "ArrowRight"
					? 1
					: event.key === "ArrowLeft"
					? -1
					: 0;
				if (step === 0) return;
				event.preventDefault();
				const index = files.indexOf(open);
				const target = files[(index + step + files.length) % files.length];
				if (target) {
					select(target.name);
					target.tab.focus();
				}
			});
		}
		select(files[0]?.name ?? "client.ts");
		peerCount = next.client === undefined ? 0 : next.peers;
		peersSection.hidden = next.client === undefined;
		const radio = examplePicker.querySelector<HTMLInputElement>(
			`input[value="${next.id}"]`,
		);
		if (radio) radio.checked = true;
		syncExtensions();
		return run();
	}

	function select(name: FileName): void {
		for (const open of files) {
			const selected = open.name === name;
			open.tab.setAttribute("aria-selected", String(selected));
			open.tab.tabIndex = selected ? 0 : -1;
			open.panel.hidden = !selected;
		}
	}

	function focusLine(name: FileName, line: number): void {
		const open = file(name);
		if (open === undefined) return;
		select(name);
		open.editor.markLine(line);
		const position = open.editor.view.state.doc.line(
			Math.min(line, open.editor.view.state.doc.lines),
		).from;
		open.editor.view.dispatch({
			selection: { anchor: position },
			scrollIntoView: true,
		});
		open.editor.view.focus();
	}

	examplePicker.addEventListener("change", (event) => {
		const id = (event.target as HTMLInputElement).value;
		const next = EXAMPLES.find((candidate) => candidate.id === id) ??
			handedOver;
		if (next) {
			void load(next);
			history.replaceState(
				null,
				"",
				next.id === "docs" ? location.pathname : `?example=${next.id}`,
			);
		}
	});

	// ---------- Running ----------

	async function run(): Promise<RunResult> {
		const token = ++runToken;
		clearTimeout(rerun);
		result = emptyResult();
		log.clear();
		preview.clear();
		serverDocSection.hidden = true;
		ports.textContent = "";
		for (const open of files) open.editor.markLine(undefined);
		for (const { peer, element } of peers) {
			peer.remove();
			element.remove();
		}
		peers = [];
		setState("running", "Running…");
		const started = performance.now();
		const waits: Promise<void>[] = [];

		const server = file("server.ts");
		if (server) {
			const listening = new Promise<void>((resolve) => {
				onListening = resolve;
			});
			const settled = sandbox.runServer("server.ts", server.editor.value);
			waits.push(settled);
			// Peers connect once the server listens, or has finished starting without listening.
			await Promise.race([settled, listening, delay(SERVER_START_MS)]);
		} else {
			sandbox.stopServer();
		}
		if (token !== runToken) return snapshot();

		for (let index = 0; index < peerCount; index++) {
			waits.push(startPeer().settled);
		}
		updatePeerButton();
		await Promise.race([Promise.all(waits), delay(RUN_TIMEOUT_MS)]);
		await quiet();
		if (token !== runToken) return snapshot();

		const elapsed = Math.round(performance.now() - started);
		if (result.errors.length > 0) {
			const [first] = result.errors;
			setState(
				"error",
				`${first?.source}: ${first?.message}${
					first?.line === undefined
						? ""
						: ` (${first.file}, line ${first.line})`
				}`,
			);
		} else {
			const listening = result.ports.length > 0
				? ` · listening on ${
					result.ports.map((port) => `localhost:${port}`).join(", ")
				}`
				: "";
			const peerText = peers.length > 0
				? ` · ${peers.length} ${peers.length === 1 ? "peer" : "peers"}`
				: "";
			setState("ok", `Ran in ${elapsed} ms${listening}${peerText}`);
		}
		return snapshot();
	}

	function startPeer(): BrowserPeer {
		const client = file("client.ts");
		if (client === undefined) {
			throw new Error("This example has no browser code");
		}
		const index = peers.length;
		const label = `peer ${index + 1}`;
		const element = document.createElement("div");
		element.className = "pg-peer";
		element.style.setProperty(
			"--peer",
			PEER_COLORS[index % PEER_COLORS.length] ?? PEER_COLORS[0]!,
		);
		const head = document.createElement("div");
		head.className = "pg-peer__head";
		const name = document.createElement("span");
		name.textContent = `Peer ${index + 1}`;
		const remove = document.createElement("button");
		remove.type = "button";
		remove.textContent = "Remove";
		remove.setAttribute("aria-label", `Remove peer ${index + 1}`);
		head.append(name, remove);
		element.append(head);
		peersGrid.append(element);
		const peer = sandbox.addBrowser(element, {
			file: "client.ts",
			source: client.editor.value,
			html: file("index.html")?.editor.value ?? "",
			title: `Peer ${index + 1}: client.ts running`,
		});
		const entry: Peer = { peer, element, label };
		peers.push(entry);
		remove.addEventListener("click", () => {
			peer.remove();
			element.remove();
			peers = peers.filter((candidate) => candidate !== entry);
			updatePeerButton();
		});
		return peer;
	}

	function updatePeerButton(): void {
		addPeerButton.disabled = file("client.ts") === undefined ||
			peers.length >= MAX_PEERS;
	}

	addPeerButton.addEventListener("click", () => {
		if (peers.length < MAX_PEERS) startPeer();
		updatePeerButton();
	});
	find<HTMLButtonElement>("[data-run]").addEventListener(
		"click",
		() => void run(),
	);
	find<HTMLButtonElement>("[data-clear]").addEventListener(
		"click",
		() => log.clear(),
	);

	async function quiet(): Promise<void> {
		const limit = performance.now() + QUIET_LIMIT_MS;
		while (
			performance.now() < limit && performance.now() - lastEvent < QUIET_MS
		) await delay(QUIET_MS / 3);
	}

	function onEvent(source: RealmSource, event: RealmEvent): void {
		lastEvent = performance.now();
		const peer = source.realm === "browser"
			? peers.find((candidate) => candidate.peer.peer === source.peer)
			: undefined;
		// A removed peer's last words don't belong to this run.
		if (source.realm === "browser" && peer === undefined) return;
		const label = peer?.label ?? "server";
		const fileName: FileName = source.realm === "server"
			? "server.ts"
			: "client.ts";
		switch (event.type) {
			case "log":
				log.log(event.level, event.text, label);
				result = {
					...result,
					logs: [...result.logs, {
						source: label,
						level: event.level,
						text: event.text,
					}],
				};
				return;
			case "uncaught":
			case "settled":
				if (event.error) failed(label, fileName, event.error);
				return;
			case "listening":
				result = { ...result, ports: [...result.ports, event.port] };
				ports.textContent = result.ports.map((port) => `localhost:${port}`)
					.join(" · ");
				onListening?.();
				return;
			case "documents":
				result = { ...result, documents: event.documents };
				serverDocSection.hidden = false;
				preview.show(event.documents);
		}
	}

	function failed(source: string, fileName: FileName, error: RunError): void {
		log.error(error, source);
		lastErrorFile = fileName;
		file(fileName)?.editor.markLine(error.line);
		const entry = error.line === undefined
			? { source, file: fileName, message: error.message }
			: { source, file: fileName, message: error.message, line: error.line };
		result = {
			...result,
			errors: [...result.errors, entry],
			logs: [...result.logs, {
				source,
				level: "uncaught",
				text: error.message,
			}],
		};
		if (root.dataset["state"] !== "running") {
			setState("error", `${source}: ${error.message}`);
		}
	}

	function setState(state: "running" | "ok" | "error", text: string): void {
		root.dataset["state"] = state;
		status.textContent = text;
	}

	function snapshot(): RunResult {
		return { ...result, ok: result.errors.length === 0 };
	}

	// ---------- Extension switches ----------

	interface Managed {
		readonly open: OpenFile;
		readonly list: ExtensionList;
	}

	function managedFiles(): { managed: Managed[]; reason: string } {
		const managed: Managed[] = [];
		let reason = "This example has no code to switch extensions in.";
		for (const open of files) {
			if (open.name === "index.html") continue;
			const read = readExtensionList(open.editor.value, catalog);
			if (read.ok) managed.push({ open, list: read.list });
			else if (managed.length === 0) reason = read.reason;
		}
		return { managed, reason };
	}

	function syncExtensions(): void {
		const { managed, reason } = managedFiles();
		const enabled = new Set(managed[0]?.list.names ?? []);
		const extrasAllowed = managed.length > 0 &&
			managed.every(({ list }) => allowsExtras(list.module));
		const extras = new Set(catalog.extras);
		for (const checkbox of checkboxes) {
			checkbox.checked = enabled.has(checkbox.value);
			checkbox.disabled = managed.length === 0 ||
				(extras.has(checkbox.value) && !extrasAllowed);
		}
		extensionCount.textContent = managed.length === 0
			? ""
			: `${enabled.size} on`;
		extensionNote.textContent = managed.length === 0
			? `${reason} This example uses the StarterKit.`
			: extrasAllowed
			? "Switching one rewrites the extension list and the import, and runs the code again."
			: "@canvist/web compiles in the StarterKit, so the rest run on the server realm: try Every extension.";
	}

	function setExtensions(names: readonly string[]): boolean {
		const { managed } = managedFiles();
		for (const { open, list } of managed) {
			open.editor.setValue(
				writeExtensionList(open.editor.value, list, names, catalog),
			);
		}
		return managed.length > 0;
	}

	for (const checkbox of checkboxes) {
		checkbox.addEventListener("change", () => {
			setExtensions(
				checkboxes.filter((candidate) => candidate.checked).map((candidate) =>
					candidate.value
				),
			);
		});
	}

	// ---------- Agents ----------

	window.canvistPlayground = {
		examples: EXAMPLES.map((candidate) => candidate.id),
		get example() {
			return example.id;
		},
		load(id) {
			const next = EXAMPLES.find((candidate) => candidate.id === id);
			if (next === undefined) {
				return Promise.reject(
					new Error(
						`No example "${id}". Try ${
							EXAMPLES.map((candidate) => candidate.id).join(", ")
						}.`,
					),
				);
			}
			return load(next);
		},
		read: (name) => file(name)?.editor.value,
		write(name, code) {
			const open = file(name);
			if (open === undefined) {
				return Promise.reject(
					new Error(`The "${example.id}" example has no ${name}`),
				);
			}
			open.editor.setValue(code);
			return run();
		},
		run,
		extensions() {
			const { managed } = managedFiles();
			return {
				enabled: managed[0]?.list.names ?? [],
				editable: managed.length > 0,
				note: extensionNote.textContent ?? "",
			};
		},
		setExtensions(names) {
			if (!setExtensions(names)) {
				return Promise.reject(
					new Error(extensionNote.textContent ?? "No extension list to change"),
				);
			}
			return run();
		},
		async addPeer() {
			if (peers.length >= MAX_PEERS) {
				throw new Error(`The playground runs up to ${MAX_PEERS} peers`);
			}
			await startPeer().settled;
			updatePeerButton();
		},
		fetch: sandbox.fetch,
		logs: () => result.logs,
		documents: () => result.documents,
	};

	// ---------- Start ----------

	const handoff = takePlaygroundHandoff();
	const handedOver: PlaygroundExample | undefined =
		new URLSearchParams(location.search).get("from") === "docs" && handoff
			? {
				id: "docs",
				title: "From the docs",
				summary: "The sample you opened, as you left it.",
				...handoff,
				peers: handoff.client === undefined ? 0 : 1,
			}
			: undefined;
	if (handedOver) addExampleChoice(examplePicker, handedOver);
	const requested = EXAMPLES.find((candidate) =>
		candidate.id === new URLSearchParams(location.search).get("example")
	);
	void load(
		handedOver ?? requested ??
			EXAMPLES.find((candidate) =>
				candidate.id === root.dataset["defaultExample"]
			) ?? example,
	);
}

/** Adds the docs sample as the first choice, chosen. */
function addExampleChoice(
	picker: HTMLFieldSetElement,
	example: PlaygroundExample,
): void {
	const label = document.createElement("label");
	label.className = "pg-example";
	const input = document.createElement("input");
	input.type = "radio";
	input.name = "example";
	input.value = example.id;
	input.checked = true;
	const title = document.createElement("span");
	title.className = "pg-example__title";
	title.textContent = example.title;
	const summary = document.createElement("span");
	summary.className = "pg-example__summary";
	summary.textContent = example.summary;
	label.append(input, title, summary);
	picker.querySelector("legend")?.after(label);
}

function emptyResult(): Omit<RunResult, "ok"> {
	return { errors: [], logs: [], documents: [], ports: [] };
}

function delay(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms));
}
