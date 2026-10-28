import type { LogLevel, RunError } from "./protocol.ts";

/**
 * The console a run prints to: one line per `console` call, errors with
 * the line they came from, and an optional label for which realm printed
 * it when several share one console.
 */
export class ConsoleView {
	readonly #list: HTMLOListElement;
	readonly #onLine: ((line: number) => void) | undefined;
	#lines = 0;

	/** `onLine` is called when someone picks an error's line number. */
	constructor(root: HTMLElement, onLine?: (line: number) => void) {
		this.#list = document.createElement("ol");
		this.#list.className = "sandbox-console";
		this.#list.setAttribute("aria-label", "Console output");
		this.#onLine = onLine;
		root.append(this.#list);
	}

	/** Every printed line, for comparing with what a sample promises. */
	get printed(): string[] {
		return [
			...this.#list.querySelectorAll<HTMLElement>(
				"[data-level=log], [data-level=info]",
			),
		].flatMap((item) =>
			(item.querySelector(".sandbox-console__text")?.textContent ?? "").split(
				"\n",
			)
		);
	}

	get empty(): boolean {
		return this.#lines === 0;
	}

	log(level: LogLevel, text: string, label?: string): void {
		const item = this.#item(level, label);
		item.append(span("sandbox-console__text", text));
		this.#push(item);
	}

	error(error: RunError, label?: string): void {
		const item = this.#item("error", label);
		item.append(span("sandbox-console__text", error.message));
		if (error.line !== undefined) {
			const line = error.line;
			const button = document.createElement("button");
			button.type = "button";
			button.className = "sandbox-console__line";
			button.textContent = `line ${line}`;
			button.addEventListener("click", () => this.#onLine?.(line));
			item.append(" ", button);
		}
		this.#push(item);
	}

	note(text: string, label?: string): void {
		const item = this.#item("note", label);
		item.append(span("sandbox-console__text", text));
		this.#push(item);
	}

	clear(): void {
		this.#list.replaceChildren();
		this.#lines = 0;
	}

	#item(level: LogLevel | "note", label: string | undefined): HTMLLIElement {
		const item = document.createElement("li");
		item.dataset["level"] = level;
		if (label !== undefined) item.append(span("sandbox-console__label", label));
		return item;
	}

	#push(item: HTMLLIElement): void {
		this.#lines++;
		const pinned = this.#list.scrollTop + this.#list.clientHeight >=
			this.#list.scrollHeight - 4;
		this.#list.append(item);
		// Follow new output, unless the reader scrolled up to read.
		if (pinned) this.#list.scrollTop = this.#list.scrollHeight;
	}
}

function span(className: string, text: string): HTMLSpanElement {
	const node = document.createElement("span");
	node.className = className;
	node.textContent = text;
	return node;
}
