import type { Realm } from "../sandbox/modules.ts";

/**
 * Run buttons on the docs' live samples (the `<pre data-live>` blocks the
 * build marked). The sandbox, its editor, and canvist's WebAssembly load
 * only when a reader reaches for a button, so pages cost nothing extra
 * until then.
 */

const loadSandbox = () => import("../sandbox/live-sample.ts");

export function enhanceLiveSamples(): void {
	for (
		const pre of document.querySelectorAll<HTMLPreElement>(
			".docs-article pre[data-live]",
		)
	) {
		const frame = pre.closest<HTMLElement>(".docs-code");
		const realm = realmOf(pre.dataset["live"]);
		if (!frame || realm === undefined) continue;

		const run = document.createElement("button");
		run.type = "button";
		run.className = "docs-run";
		run.textContent = "Run";
		run.setAttribute("aria-label", "Run this sample, and edit it live");
		// Start downloading as the pointer or focus arrives, so pressing feels instant.
		const warm = () => void loadSandbox();
		run.addEventListener("pointerenter", warm, { once: true });
		run.addEventListener("focus", warm, { once: true });
		run.addEventListener("click", async () => {
			run.disabled = true;
			run.textContent = "Loading…";
			try {
				const { mountLiveSample } = await loadSandbox();
				run.remove();
				mountLiveSample({ frame, pre, realm, html: markupBefore(frame) });
			} catch (error) {
				console.error(error);
				run.disabled = false;
				run.textContent = "Retry";
			}
		});
		actionsOf(frame).prepend(run);
	}
}

/** The row of buttons in a code block's corner, made on first use. */
export function actionsOf(frame: HTMLElement): HTMLElement {
	let actions = frame.querySelector<HTMLElement>(
		":scope > .docs-code__actions",
	);
	if (actions === null) {
		actions = document.createElement("div");
		actions.className = "docs-code__actions";
		frame.append(actions);
	}
	return actions;
}

function realmOf(value: string | undefined): Realm | undefined {
	return value === "server" || value === "browser" ? value : undefined;
}

/** The `html` sample a browser sample runs against: the nearest one before it in its section. */
function markupBefore(frame: HTMLElement): string | undefined {
	for (
		let node = frame.previousElementSibling;
		node !== null;
		node = node.previousElementSibling
	) {
		if (/^H[1-4]$/.test(node.tagName)) return undefined;
		const html = node.matches(".docs-code")
			? node.querySelector<HTMLElement>('pre[data-language="html"]')
			: null;
		if (html) return html.innerText.replace(/\n$/, "");
	}
	return undefined;
}
