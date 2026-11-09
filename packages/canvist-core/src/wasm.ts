/**
 * What the typed classes need from a canvist WebAssembly module. Every
 * module canvist ships (StarterKit, full, collaborating) exports classes
 * with at least these methods, so {@link Engine} and {@link Doc} work over
 * any of them without loading one.
 *
 * @module
 */

/** A module's engine: a configured set of extensions. */
export interface WasmEngine {
	metadata(): string;
	create(): WasmDoc;
	parse(format: string, input: string): WasmDoc;
	restore(snapshot: string): WasmDoc;
	free(): void;
}

/** A module's document and its editor. */
export interface WasmDoc {
	run(name: string, args: string): boolean;
	can(name: string, args: string): boolean;
	query(name: string, args: string): string;
	export(format: string): string;
	selection(): string;
	select(
		anchorPath: Uint32Array,
		anchorOffset: number,
		headPath: Uint32Array,
		headOffset: number,
	): void;
	selectAll(): void;
	insertText(text: string): boolean;
	typeText(text: string): boolean;
	deleteSelection(): boolean;
	press(key: string): boolean;
	paste(format: string, content: string): boolean;
	undo(): boolean;
	redo(): boolean;
	snapshot(): string;
	free(): void;
}
