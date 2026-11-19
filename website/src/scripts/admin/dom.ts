/**
 * Building the dashboard's DOM from data. Text always goes in as text
 * nodes, never as HTML, so nothing a visitor or Stripe sends can run.
 */

type Child = Node | string | number | null | undefined | false;
type Attributes = Record<string, string | number | boolean | undefined>;

function apply(
	element: Element,
	attributes: Attributes,
	children: Child[],
): void {
	for (const [name, value] of Object.entries(attributes)) {
		if (value === undefined || value === false) continue;
		element.setAttribute(name, value === true ? "" : String(value));
	}
	for (const child of children) {
		if (child === null || child === undefined || child === false) continue;
		element.append(typeof child === "number" ? String(child) : child);
	}
}

/** An HTML element with `attributes` and `children`. */
export function h<K extends keyof HTMLElementTagNameMap>(
	tag: K,
	attributes: Attributes = {},
	...children: Child[]
): HTMLElementTagNameMap[K] {
	const element = document.createElement(tag);
	apply(element, attributes, children);
	return element;
}

/** An SVG element with `attributes` and `children`. */
export function svg<K extends keyof SVGElementTagNameMap>(
	tag: K,
	attributes: Attributes = {},
	...children: Child[]
): SVGElementTagNameMap[K] {
	const element = document.createElementNS("http://www.w3.org/2000/svg", tag);
	apply(element, attributes, children);
	return element;
}

/** The element matching `selector` under `root`; missing markup is a bug. */
export function need<T extends Element>(root: ParentNode, selector: string): T {
	const element = root.querySelector<T>(selector);
	if (!element) throw new Error(`admin markup is missing ${selector}`);
	return element;
}

/** Show `message` in a live region, as an error or a confirmation. */
export function announce(
	region: HTMLElement,
	message: string,
	tone: "info" | "error" = "info",
): void {
	region.dataset["tone"] = tone;
	region.textContent = message;
}
