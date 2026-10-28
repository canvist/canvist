/**
 * `node:events`: the `EventEmitter` Node's servers and sockets are built on,
 * with the behaviour code relies on. An `error` event nobody listens to
 * throws, as in Node.
 */

type Listener = (...args: never[]) => void;

export class EventEmitter {
	readonly #listeners = new Map<string | symbol, Listener[]>();

	on(event: string | symbol, listener: Listener): this {
		this.#listeners.set(event, [
			...(this.#listeners.get(event) ?? []),
			listener,
		]);
		return this;
	}

	addListener(event: string | symbol, listener: Listener): this {
		return this.on(event, listener);
	}

	once(event: string | symbol, listener: Listener): this {
		const once = (...args: never[]) => {
			this.off(event, once);
			listener(...args);
		};
		return this.on(event, once);
	}

	off(event: string | symbol, listener: Listener): this {
		const remaining = (this.#listeners.get(event) ?? []).filter((candidate) =>
			candidate !== listener
		);
		if (remaining.length === 0) this.#listeners.delete(event);
		else this.#listeners.set(event, remaining);
		return this;
	}

	removeListener(event: string | symbol, listener: Listener): this {
		return this.off(event, listener);
	}

	removeAllListeners(event?: string | symbol): this {
		if (event === undefined) this.#listeners.clear();
		else this.#listeners.delete(event);
		return this;
	}

	emit(event: string | symbol, ...args: unknown[]): boolean {
		const listeners = this.#listeners.get(event);
		if (listeners === undefined) {
			if (event === "error") {
				throw args[0] instanceof Error ? args[0] : new Error(String(args[0]));
			}
			return false;
		}
		for (const listener of listeners) {
			(listener as (...args: unknown[]) => void).apply(this, args);
		}
		return true;
	}

	listenerCount(event: string | symbol): number {
		return this.#listeners.get(event)?.length ?? 0;
	}

	listeners(event: string | symbol): Listener[] {
		return [...(this.#listeners.get(event) ?? [])];
	}
}

export default EventEmitter;
