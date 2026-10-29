import { ConsoleView } from "./console-view.ts";
import { createCodeEditor } from "./editor.ts";
import { expectedOutput } from "./expected.ts";
import { savePlaygroundHandoff } from "./handoff.ts";
import { type BrowserPeer, type RealmSource, Sandbox } from "./host.ts";
import type { Realm } from "./modules.ts";
import { DocumentPreview } from "./preview.ts";
import type { RealmEvent } from "./protocol.ts";

/**
 * A docs sample, made live: its code in an editor, run on the page as it
 * changes, with what it printed and the document it made beneath it.
 * Server samples run in a worker; browser samples run in a frame, against
 * the markup in the `html` sample before them.
 */

const RERUN_DELAY_MS = 600;
const DEFAULT_MARKUP = '<div id="editor"></div>';

export interface LiveSampleOptions {
	/** The `.docs-code` frame around the sample. */
	readonly frame: HTMLElement;
	readonly pre: HTMLPreElement;
	readonly realm: Realm;
	/** The markup the sample runs against, for browser samples. */
	readonly html: string | undefined;
}

export function mountLiveSample(options: LiveSampleOptions): void {
	const { frame, pre, realm } = options;
	const original = pre.innerText.replace(/\n$/, "");
	const html = withEditorElement(options.html ?? "");
	const file = realm === "server" ? "sample.ts" : "client.ts";

	frame.classList.add("docs-live");
	frame.dataset["realm"] = realm;
	const editorHost = element("div", "docs-live__editor");
	const bar = element("div", "docs-live__bar");
	const status = element("p", "docs-live__status");
	status.setAttribute("role", "status");
	const reset = button("Reset", "docs-live__action");
	const open = button("Open in playground", "docs-live__action");
	bar.append(status, reset, open);
	const output = element("div", "docs-live__output");
	const consoleHost = element("div", "docs-live__console");
	const previewHost = element("div", "");
	const browserHost = element("div", "docs-live__browser");
	output.append(
		...(realm === "server"
			? [consoleHost, previewHost]
			: [browserHost, consoleHost]),
	);

	// The highlighted block stays, hidden, so the Copy button copies the code as edited.
	pre.hidden = true;
	pre.after(editorHost, bar, output);

	const sandbox = new Sandbox(onEvent, { fitFrames: true });
	const preview = new DocumentPreview(previewHost, { editable: true });
	const log = new ConsoleView(consoleHost, (line) => {
		editor.markLine(line);
		editor.view.focus();
	});
	let peer: BrowserPeer | undefined;
	let run = 0;
	let started = 0;
	let sawDocuments = false;
	let timer: ReturnType<typeof setTimeout> | undefined;

	const editor = createCodeEditor(editorHost, {
		doc: original,
		language: "ts",
		label: "Sample code, live: it runs as you edit",
		onRun: () => start(),
		onChange: (value) => {
			pre.querySelector("code")!.textContent = value;
			clearTimeout(timer);
			timer = setTimeout(start, RERUN_DELAY_MS);
		},
	});

	reset.addEventListener("click", () => {
		editor.setValue(original);
		start();
	});
	open.addEventListener("click", () => {
		savePlaygroundHandoff(
			realm === "server"
				? { server: editor.value }
				: { client: editor.value, html },
		);
		location.assign("/playground?from=docs");
	});

	function start(): void {
		clearTimeout(timer);
		run++;
		started = performance.now();
		sawDocuments = false;
		log.clear();
		editor.markLine(undefined);
		setStatus("running", "Running…");
		if (realm === "server") {
			void sandbox.runServer(file, editor.value);
			return;
		}
		peer?.remove();
		peer = sandbox.addBrowser(browserHost, {
			file,
			source: editor.value,
			html,
			title: "Live sample",
		});
	}

	function onEvent(source: RealmSource, event: RealmEvent): void {
		// A browser sample's frame stays after it settles, and can keep printing.
		if (source.realm === "browser" && source.peer !== peer?.peer) return;
		switch (event.type) {
			case "log":
				log.log(event.level, event.text);
				return;
			case "uncaught":
				log.error(event.error);
				editor.markLine(event.error.line);
				return;
			case "documents":
				sawDocuments = true;
				preview.show(event.documents);
				return;
			case "listening":
				log.note(`Listening on localhost:${event.port} in this tab`);
				return;
			case "settled":
				settled(event.error);
		}
	}

	function settled(
		error: { message: string; line?: number } | undefined,
	): void {
		const thisRun = run;
		const elapsed = Math.round(performance.now() - started);
		if (realm === "server" && !sawDocuments) preview.clear();
		if (error) {
			log.error(error);
			editor.markLine(error.line);
			setStatus(
				"error",
				error.line === undefined ? "Failed" : `Failed on line ${error.line}`,
			);
			return;
		}
		const expected = expectedOutput(editor.value);
		const printed = log.printed;
		if (expected === undefined) setStatus("ok", `Ran in ${elapsed} ms`);
		else if (printed.join("\n").trimEnd() === expected.join("\n")) {
			setStatus("ok", `Ran in ${elapsed} ms and printed what the comments say`);
		} else {setStatus(
				"differs",
				`Ran in ${elapsed} ms, but printed something other than the comments say`,
			);}
		if (thisRun === run && log.empty && realm === "server" && !sawDocuments) {
			log.note("Nothing printed");
		}
	}

	function setStatus(
		state: "running" | "ok" | "differs" | "error",
		text: string,
	): void {
		frame.dataset["state"] = state;
		status.textContent = text;
	}

	start();
	editor.view.focus();
}

/** The sample's markup, with the `#editor` element browser samples query for when it has none. */
function withEditorElement(markup: string): string {
	return /\bid=["']editor["']/.test(markup)
		? markup
		: `${markup}${markup ? "\n" : ""}${DEFAULT_MARKUP}`;
}

function element<K extends keyof HTMLElementTagNameMap>(
	tag: K,
	className: string,
): HTMLElementTagNameMap[K] {
	const node = document.createElement(tag);
	if (className) node.className = className;
	return node;
}

function button(text: string, className: string): HTMLButtonElement {
	const node = element("button", className);
	node.type = "button";
	node.textContent = text;
	return node;
}
