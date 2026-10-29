import { core, starterKit } from "@canvist/headless/extensions";
import type { Editor, Extensions } from "@canvist/web";
import { pageScheme } from "./host.ts";
import type { DocSnapshot } from "./protocol.ts";

/**
 * Shows a document the code made: in the real editor, which people can type
 * in when the preview is editable, and as its HTML, JSON, and Markdown
 * exports. Documents with extensions beyond the StarterKit load
 * `@canvist/web/full`, a larger build; the rest load `@canvist/web`.
 */

type View = "editor" | "html" | "json" | "markdown";

const VIEWS: readonly { readonly view: View; readonly label: string }[] = [
	{ view: "editor", label: "Editor" },
	{ view: "html", label: "HTML" },
	{ view: "json", label: "JSON" },
	{ view: "markdown", label: "Markdown" },
];

/**
 * The exported HTML shows without scripts or network access: an export can
 * embed a video or an image from anywhere, and the preview loads none.
 */
const HTML_PREVIEW_HEAD =
	`<meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src data:"><style>
:root { color-scheme: light dark; }
body { margin: 0; padding: 1rem; background: light-dark(#fff, #201c2b); color: light-dark(#17141f, #f5f3ff); font: 1rem/1.55 ui-sans-serif, system-ui, sans-serif; }
body > :first-child { margin-top: 0; }
body > :last-child { margin-bottom: 0; }
h1, h2, h3, h4 { line-height: 1.15; margin: 1.2em 0 0.4em; }
p, ul, ol, blockquote, pre, table, details, aside { margin: 0 0 0.75em; }
blockquote, aside { padding-left: 0.875rem; border-left: 3px solid light-dark(#c4bbff, #4b3fa8); }
code, pre { font-family: ui-monospace, "SF Mono", Menlo, monospace; font-size: 0.875em; }
pre { padding: 0.75rem; border-radius: 0.5rem; background: light-dark(#f5f3ff, #17141f); overflow-x: auto; }
mark { background: light-dark(#fff3a3, #6b5d00); color: inherit; }
table { border-collapse: collapse; }
th, td { padding: 0.375rem 0.625rem; border: 1px solid light-dark(#dedaf2, #2e2940); text-align: left; }
a { color: light-dark(#3b2cf5, #8a80ff); }
img, video, iframe { max-width: 100%; }
ul[data-type="taskList"] { padding-left: 0.25rem; list-style: none; }
li[data-type="taskItem"]::before { content: "\\2610"; margin-right: 0.5em; }
li[data-type="taskItem"][data-checked="true"]::before { content: "\\2611"; }
li[data-type="taskItem"] > p { display: inline; }
[data-callout-type] { margin: 0 0 0.75em; padding: 0.625rem 0.875rem; border-radius: 0.5rem; background: light-dark(#f5f3ff, #17141f); border-left: 3px solid light-dark(#3b2cf5, #8a80ff); }
[data-type="inline-math"], [data-type="block-math"] { font-family: ui-monospace, "SF Mono", Menlo, monospace; font-size: 0.875em; }
</style>`;

export interface PreviewOptions {
	/**
	 * Whether people can type in the Editor tab. The other tabs then show
	 * the edited document, until the code runs again and replaces it.
	 */
	readonly editable?: boolean;
}

let nextPreview = 1;

export class DocumentPreview {
	readonly #root: HTMLElement;
	readonly #tabs = new Map<View, HTMLButtonElement>();
	readonly #panel: HTMLElement;
	/** The editor stays here while another tab shows, so edits survive switching tabs. */
	readonly #editorHost: HTMLElement;
	readonly #viewHost: HTMLElement;
	readonly #hint: HTMLElement;
	readonly #picker: HTMLSelectElement;
	readonly #pickerLabel: HTMLElement;
	readonly #id = nextPreview++;
	#editable: boolean;
	#documents: readonly DocSnapshot[] = [];
	#chosen: number | undefined;
	#view: View = "editor";
	/** The editor, and the snapshot it shows: the other tabs read the editor while it shows the chosen one. */
	#editor:
		| {
			readonly editor: Editor<Extensions>;
			readonly key: string;
			snapshot: DocSnapshot;
		}
		| undefined;
	#rendering = Promise.resolve();

	constructor(root: HTMLElement, options: PreviewOptions = {}) {
		this.#root = root;
		this.#editable = options.editable ?? false;
		root.classList.add("sandbox-preview");
		root.hidden = true;

		const bar = element("div", "sandbox-preview__bar");
		const tablist = element("div", "sandbox-preview__tabs");
		tablist.setAttribute("role", "tablist");
		tablist.setAttribute("aria-label", "Preview as");
		for (const { view, label } of VIEWS) {
			const tab = element("button", "sandbox-preview__tab");
			tab.type = "button";
			tab.textContent = label;
			tab.id = `sandbox-preview-${this.#id}-${view}`;
			tab.setAttribute("role", "tab");
			tab.setAttribute("aria-controls", `sandbox-preview-${this.#id}-panel`);
			tab.addEventListener("click", () => this.#select(view, true));
			tab.addEventListener("keydown", (event) => this.#arrow(event, view));
			this.#tabs.set(view, tab);
			tablist.append(tab);
		}

		this.#pickerLabel = element("label", "sandbox-preview__picker");
		this.#pickerLabel.append("Document ");
		this.#picker = element("select", "");
		this.#picker.addEventListener("change", () => {
			this.#chosen = Number(this.#picker.value);
			this.#render();
		});
		this.#pickerLabel.append(this.#picker);
		bar.append(tablist, this.#pickerLabel);

		this.#panel = element("div", "sandbox-preview__panel");
		this.#panel.id = `sandbox-preview-${this.#id}-panel`;
		this.#panel.setAttribute("role", "tabpanel");
		this.#editorHost = element("div", "sandbox-preview__editor");
		this.#viewHost = element("div", "");
		this.#panel.append(this.#editorHost, this.#viewHost);
		this.#hint = element(
			"p",
			"sandbox-preview__hint",
			"Click in to type: the HTML, JSON, and Markdown tabs follow your edits.",
		);
		this.#hint.hidden = true;
		root.append(bar, this.#hint, this.#panel);
	}

	/** Shows the most recent of `documents`, or the one picked earlier if it's still there. */
	show(documents: readonly DocSnapshot[]): void {
		this.#documents = documents;
		this.#root.hidden = documents.length === 0;
		if (documents.length === 0) return;
		if (!documents.some((snapshot) => snapshot.id === this.#chosen)) {
			this.#chosen = undefined;
		}
		this.#picker.replaceChildren(
			...[...documents].sort((a, b) => a.id - b.id).map((snapshot) => {
				const option = document.createElement("option");
				option.value = String(snapshot.id);
				option.textContent = `${snapshot.id} of ${documents.length}`;
				return option;
			}),
		);
		this.#picker.value = String(this.#current()?.id ?? "");
		this.#pickerLabel.hidden = documents.length < 2;
		this.#render();
	}

	clear(): void {
		this.show([]);
	}

	/** Whether people can type in the Editor tab; see {@link PreviewOptions.editable}. */
	setEditable(editable: boolean): void {
		this.#editable = editable;
		this.#editor?.editor.setEditable(editable);
		this.#hint.hidden = !editable || this.#view !== "editor";
	}

	destroy(): void {
		this.#editor?.editor.destroy();
		this.#editor = undefined;
		this.#root.replaceChildren();
	}

	#current(): DocSnapshot | undefined {
		return this.#documents.find((snapshot) => snapshot.id === this.#chosen) ??
			this.#documents[0];
	}

	#select(view: View, focus: boolean): void {
		this.#view = view;
		this.#render();
		if (focus) this.#tabs.get(view)?.focus();
	}

	#arrow(event: KeyboardEvent, from: View): void {
		const index = VIEWS.findIndex(({ view }) => view === from);
		const step = event.key === "ArrowRight"
			? 1
			: event.key === "ArrowLeft"
			? -1
			: 0;
		if (step === 0 && event.key !== "Home" && event.key !== "End") return;
		event.preventDefault();
		const next = event.key === "Home"
			? 0
			: event.key === "End"
			? VIEWS.length - 1
			: (index + step + VIEWS.length) % VIEWS.length;
		const view = VIEWS[next]?.view;
		if (view) this.#select(view, true);
	}

	#render(): void {
		const snapshot = this.#current();
		if (snapshot === undefined) return;
		const view = this.#view;
		for (const [name, tab] of this.#tabs) {
			const selected = name === view;
			tab.setAttribute("aria-selected", String(selected));
			tab.tabIndex = selected ? 0 : -1;
		}
		this.#panel.setAttribute("aria-labelledby", this.#tabs.get(view)!.id);
		this.#panel.dataset["view"] = view;
		this.#hint.hidden = !this.#editable || view !== "editor";
		// The editor takes focus itself; the other views need the panel to.
		this.#panel.tabIndex = view === "editor" ? -1 : 0;
		// One render at a time: the editor loads asynchronously the first time.
		this.#rendering = this.#rendering.then(() =>
			this.#renderView(view, snapshot)
		).catch((error: unknown) => {
			this.#editorHost.hidden = true;
			this.#viewHost.hidden = false;
			this.#viewHost.replaceChildren(
				element(
					"p",
					"sandbox-preview__note",
					`The editor couldn't start: ${String(error)}`,
				),
			);
		});
	}

	async #renderView(view: View, snapshot: DocSnapshot): Promise<void> {
		this.#editorHost.hidden = view !== "editor";
		this.#viewHost.hidden = view === "editor";
		if (view === "editor") {
			this.#viewHost.replaceChildren();
			await this.#paint(snapshot);
			return;
		}
		const shown = this.#exports(snapshot);
		switch (view) {
			case "html":
				this.#viewHost.replaceChildren(htmlFrame(shown.html));
				return;
			case "json":
				this.#viewHost.replaceChildren(
					source(JSON.stringify(shown.json, null, 2)),
				);
				return;
			case "markdown":
				this.#viewHost.replaceChildren(source(shown.markdown));
		}
	}

	/** The document as the editor has it, edits included, or as the code made it. */
	#exports(
		snapshot: DocSnapshot,
	): Pick<DocSnapshot, "html" | "json" | "markdown"> {
		const editor = this.#editor;
		if (editor === undefined || editor.snapshot !== snapshot) return snapshot;
		return {
			html: editor.editor.export("html"),
			json: editor.editor.toJson(),
			markdown: editor.editor.export("markdown"),
		};
	}

	async #paint(snapshot: DocSnapshot): Promise<void> {
		const full = !isStarterKit(snapshot.extensions);
		// Every editor has the core extensions; the rest are the ones the document used.
		const key = `${full ? "full" : "web"}|${
			[...snapshot.extensions].sort().join(",")
		}|${pageScheme()}`;
		const content = snapshot.json as Parameters<
			Editor<Extensions>["setContent"]
		>[0];
		const current = this.#editor;
		if (current?.key === key) {
			if (current.snapshot !== snapshot) {
				current.editor.setContent(content);
				current.snapshot = snapshot;
			}
			return;
		}
		current?.editor.destroy();
		this.#editor = undefined;
		const host = element("div", "");
		this.#editorHost.replaceChildren(host);
		// Extensions beyond the StarterKit need the full build, a larger download.
		const [createEditor, available]: [
			typeof import("@canvist/web").createEditor,
			Extensions,
		] = full
			? await import("@canvist/web/full").then((web) => [
				web.createEditor,
				web.allExtensions,
			])
			: await import("@canvist/web").then((web) => [
				web.createEditor,
				web.starterKit,
			]);
		const extensions = available.filter((extension) =>
			snapshot.extensions.includes(extension.name)
		);
		const editor = await createEditor(host, {
			extensions,
			content,
			theme: { scheme: pageScheme() },
			label: "The document the code made",
			editable: this.#editable,
		});
		this.#editor = {
			editor: editor as unknown as Editor<Extensions>,
			key,
			snapshot,
		};
	}
}

const STARTER_KIT_NAMES: ReadonlySet<string> = new Set(
	[...core, ...starterKit].map((extension) => extension.name),
);

/** Whether a document with `extensions` paints in `@canvist/web`, which has the StarterKit only, rather than `@canvist/web/full`. */
function isStarterKit(extensions: readonly string[]): boolean {
	return extensions.every((name) => STARTER_KIT_NAMES.has(name));
}

function htmlFrame(html: string): HTMLIFrameElement {
	const frame = element("iframe", "sandbox-preview__html");
	frame.title = "The document's HTML export";
	// Same origin, so the page can fit the frame to its content; no scripts.
	frame.setAttribute("sandbox", "allow-same-origin");
	frame.srcdoc =
		`<!doctype html><html><head>${HTML_PREVIEW_HEAD}</head><body>${html}</body></html>`;
	frame.addEventListener("load", () => {
		const height = frame.contentDocument?.documentElement.scrollHeight;
		if (height) frame.style.height = `${height}px`;
	});
	return frame;
}

function source(text: string): HTMLPreElement {
	const pre = element("pre", "sandbox-preview__source");
	pre.textContent = text;
	pre.tabIndex = 0;
	return pre;
}

function element<K extends keyof HTMLElementTagNameMap>(
	tag: K,
	className: string,
	text?: string,
): HTMLElementTagNameMap[K] {
	const node = document.createElement(tag);
	if (className) node.className = className;
	if (text !== undefined) node.textContent = text;
	return node;
}
