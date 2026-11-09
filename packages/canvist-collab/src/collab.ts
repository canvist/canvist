/**
 * Engines whose documents are Yjs peers: they join shared state, merge
 * updates from other peers, and serve y-websocket's sync protocol.
 *
 * @module
 */
import {
	Doc,
	Engine,
	type Extensions,
	type WasmDoc,
	type WasmEngine,
} from "@canvist/core";

/** A collaborating module's engine. */
export interface WasmCollabEngine extends WasmEngine {
	fromYjs(state: Uint8Array, clientId: number): WasmCollabDoc;
}

/** A collaborating module's document. */
export interface WasmCollabDoc extends WasmDoc {
	share(clientId: number): void;
	applyUpdate(update: Uint8Array): boolean;
	encodeState(): Uint8Array;
	stateVector(): Uint8Array;
	encodeDiff(stateVector: Uint8Array): Uint8Array;
	takeUpdates(): Uint8Array[];
	validate(): string;
	syncStart(): Uint8Array;
	receiveSync(message: Uint8Array): [Uint8Array[], Uint8Array[], boolean];
	flushSync(): Uint8Array[];
}

/** What {@link CollabDoc.receiveSync} says to send. */
export interface SyncReply {
	/** Messages for the connection the message came from. */
	readonly reply: readonly Uint8Array[];
	/** Messages for every other connection to the document. */
	readonly broadcast: readonly Uint8Array[];
	/** Whether the document changed. */
	readonly changed: boolean;
}

/** How well a shared document's state fits the schema. */
export interface Validation {
	/** Shared items the schema rejects; they are hidden from the document but kept. */
	readonly rejectedBlocks: number;
	/** Whether updates wait for earlier updates that have not arrived. */
	readonly waiting: boolean;
}

/** An engine whose documents collaborate. Create one with `createEngine`. */
export class CollabEngine<E extends Extensions>
	extends Engine<E, CollabDoc<E>> {
	readonly #wasm: WasmCollabEngine;

	/** @internal Use `createEngine`, which loads the WebAssembly module first. */
	constructor(wasm: WasmCollabEngine, extensions: E) {
		// The module's engine creates the module's documents.
		super(
			wasm,
			extensions,
			(doc, registered) => new CollabDoc(doc as WasmCollabDoc, registered),
		);
		this.#wasm = wasm;
	}

	/** Join a shared document from v1 Yjs state as yrs client `clientId`. */
	fromYjs(state: Uint8Array, clientId: number): CollabDoc<E> {
		return new CollabDoc(this.#wasm.fromYjs(state, clientId), this.extensions);
	}
}

/** A document that can be a Yjs peer. Every call is one undo step. */
export class CollabDoc<E extends Extensions> extends Doc<E> {
	readonly #wasm: WasmCollabDoc;

	/** @internal Documents come from a {@link CollabEngine}. */
	constructor(wasm: WasmCollabDoc, extensions: Extensions) {
		super(wasm, extensions);
		this.#wasm = wasm;
	}

	/** Start sharing the document as yrs client `clientId`. */
	share(clientId: number): void {
		this.#wasm.share(clientId);
	}

	/** Apply a v1 Yjs update from another peer; returns whether the document changed. */
	applyUpdate(update: Uint8Array): boolean {
		return this.#wasm.applyUpdate(update);
	}

	/** The whole shared state as one v1 update. */
	encodeState(): Uint8Array {
		return this.#wasm.encodeState();
	}

	/** The v1 state vector. */
	stateVector(): Uint8Array {
		return this.#wasm.stateVector();
	}

	/** Everything a peer at `stateVector` is missing, as one v1 update. */
	encodeDiff(stateVector: Uint8Array): Uint8Array {
		return this.#wasm.encodeDiff(stateVector);
	}

	/** Drain this peer's own v1 updates for broadcasting. */
	takeUpdates(): Uint8Array[] {
		return this.#wasm.takeUpdates();
	}

	/** Check the shared state against the schema. */
	validate(): Validation {
		return JSON.parse(this.#wasm.validate());
	}

	/** The y-sync message to greet a connecting client with. */
	syncStart(): Uint8Array {
		return this.#wasm.syncStart();
	}

	/** Handle one y-sync message from a client connection. */
	receiveSync(message: Uint8Array): SyncReply {
		const [reply, broadcast, changed] = this.#wasm.receiveSync(message);
		return { reply, broadcast, changed };
	}

	/** This peer's changes since the last call, as y-sync messages for every connection. */
	flushSync(): Uint8Array[] {
		return this.#wasm.flushSync();
	}
}
