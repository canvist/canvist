/**
 * An editor from `@canvist/web/full` with every extension, for the full
 * build's tests: `?markdown=` sets the document. `window.fullEditor` is
 * the editor, once it has painted.
 */
import { allExtensions, createEditor } from "../../src/full.ts";

const params = new URLSearchParams(location.search);
const editor = await createEditor(
	document.querySelector<HTMLElement>("#host")!,
	{
		extensions: allExtensions,
		content: { format: "markdown", input: params.get("markdown") ?? "" },
		label: "Test editor",
	},
);
await editor.ready();

Object.assign(globalThis, { fullEditor: editor });
