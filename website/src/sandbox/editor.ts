import { closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
import {
	defaultKeymap,
	history,
	historyKeymap,
	indentWithTab,
} from "@codemirror/commands";
import { html } from "@codemirror/lang-html";
import { javascript } from "@codemirror/lang-javascript";
import {
	bracketMatching,
	HighlightStyle,
	indentOnInput,
	indentUnit,
	syntaxHighlighting,
} from "@codemirror/language";
import {
	EditorState,
	type Extension,
	StateEffect,
	StateField,
} from "@codemirror/state";
import {
	Decoration,
	type DecorationSet,
	drawSelection,
	EditorView,
	highlightActiveLine,
	keymap,
	lineNumbers,
} from "@codemirror/view";
import { tags } from "@lezer/highlight";

/**
 * The code editor the live samples and the playground share: CodeMirror,
 * coloured like the docs' highlighted code (GitHub's light and dark
 * themes), with tabs for indentation. Escape, then Tab, moves focus on.
 */

export type Language = "ts" | "html";

export interface CodeEditorOptions {
	readonly doc: string;
	readonly language: Language;
	/** The editor's accessible name. */
	readonly label: string;
	/** Called after every change, with the new text. */
	readonly onChange?: (value: string) => void;
	/** Mod-Enter. */
	readonly onRun?: () => void;
}

export interface CodeEditor {
	readonly view: EditorView;
	readonly value: string;
	/**
	 * Changes the text to `value` as one undoable change, touching only the
	 * part that differs, so the cursor stays where it was.
	 */
	setValue(value: string): void;
	/** Shows the line an error came from, or clears it with `undefined`. */
	markLine(line: number | undefined): void;
	destroy(): void;
}

const highlight = HighlightStyle.define([
	{
		tag: [
			tags.keyword,
			tags.modifier,
			tags.operatorKeyword,
			tags.controlKeyword,
			tags.definitionKeyword,
			tags.moduleKeyword,
		],
		color: "var(--code-keyword)",
	},
	{
		tag: [
			tags.string,
			tags.special(tags.string),
			tags.regexp,
			tags.attributeValue,
		],
		color: "var(--code-string)",
	},
	{
		tag: [tags.comment, tags.lineComment, tags.blockComment],
		color: "var(--code-comment)",
	},
	{
		tag: [
			tags.function(tags.variableName),
			tags.function(tags.propertyName),
			tags.typeName,
			tags.className,
			tags.attributeName,
		],
		color: "var(--code-function)",
	},
	{
		tag: [
			tags.number,
			tags.bool,
			tags.null,
			tags.atom,
			tags.constant(tags.variableName),
		],
		color: "var(--code-constant)",
	},
	{ tag: [tags.tagName, tags.angleBracket], color: "var(--code-tag)" },
]);

const theme = EditorView.theme({
	"&": {
		color: "var(--text)",
		backgroundColor: "var(--raised)",
		fontSize: "0.8125rem",
	},
	".cm-scroller": {
		fontFamily: "var(--font-mono)",
		lineHeight: "1.7",
	},
	".cm-content": { padding: "var(--s-3) 0", caretColor: "var(--accent)" },
	".cm-gutters": {
		backgroundColor: "transparent",
		color: "var(--text-muted)",
		border: "none",
		paddingLeft: "var(--s-2)",
	},
	".cm-activeLine": { backgroundColor: "transparent" },
	"&.cm-focused .cm-activeLine": {
		backgroundColor: "color-mix(in oklab, var(--accent) 6%, transparent)",
	},
	".cm-line.cm-error-line": {
		backgroundColor: "color-mix(in oklab, var(--error) 14%, transparent)",
	},
	"&.cm-focused": { outline: "none" },
	"&.cm-focused .cm-cursor": { borderLeftColor: "var(--accent)" },
	".cm-selectionBackground, &.cm-focused .cm-selectionBackground": {
		backgroundColor:
			"color-mix(in oklab, var(--accent) 22%, transparent) !important",
	},
	".cm-matchingBracket": {
		backgroundColor: "color-mix(in oklab, var(--accent) 16%, transparent)",
		outline: "none",
	},
});

const setErrorLine = StateEffect.define<number | undefined>();
const errorLine = StateField.define<DecorationSet>({
	create: () => Decoration.none,
	update(lines, transaction) {
		for (const effect of transaction.effects) {
			if (!effect.is(setErrorLine)) continue;
			const line = effect.value;
			if (
				line === undefined || line < 1 || line > transaction.state.doc.lines
			) return Decoration.none;
			const from = transaction.state.doc.line(line).from;
			return Decoration.set([
				Decoration.line({ class: "cm-error-line" }).range(from),
			]);
		}
		// Editing clears the mark: the line it pointed at may be gone.
		return transaction.docChanged ? Decoration.none : lines;
	},
	provide: (field) => EditorView.decorations.from(field),
});

export function createCodeEditor(
	parent: HTMLElement,
	options: CodeEditorOptions,
): CodeEditor {
	const extensions: Extension[] = [
		lineNumbers(),
		history(),
		drawSelection(),
		highlightActiveLine(),
		indentOnInput(),
		bracketMatching(),
		closeBrackets(),
		indentUnit.of("\t"),
		EditorState.tabSize.of(2),
		syntaxHighlighting(highlight),
		theme,
		errorLine,
		options.language === "ts" ? javascript({ typescript: true }) : html(),
		keymap.of([
			{
				key: "Mod-Enter",
				run: () => (options.onRun?.(), options.onRun !== undefined),
			},
			...closeBracketsKeymap,
			...defaultKeymap,
			...historyKeymap,
			indentWithTab,
		]),
		// The content is focusable already; saying so lets checkers see the scroller is reachable.
		EditorView.contentAttributes.of({
			"aria-label": options.label,
			tabindex: "0",
		}),
		EditorView.updateListener.of((update) => {
			if (update.docChanged) options.onChange?.(update.state.doc.toString());
		}),
	];
	const view = new EditorView({
		parent,
		state: EditorState.create({ doc: options.doc, extensions }),
	});
	return {
		view,
		get value() {
			return view.state.doc.toString();
		},
		setValue(value) {
			const current = view.state.doc.toString();
			if (current === value) return;
			let start = 0;
			while (
				start < current.length && start < value.length &&
				current[start] === value[start]
			) start++;
			let end = 0;
			while (
				end < current.length - start && end < value.length - start &&
				current[current.length - 1 - end] === value[value.length - 1 - end]
			) end++;
			view.dispatch({
				changes: {
					from: start,
					to: current.length - end,
					insert: value.slice(start, value.length - end),
				},
			});
		},
		markLine(line) {
			view.dispatch({ effects: setErrorLine.of(line) });
		},
		destroy() {
			view.destroy();
		},
	};
}
