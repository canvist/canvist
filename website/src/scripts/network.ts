/**
 * Deciding what a reader's connection and device can afford. Kept free of
 * DOM side effects so the rules are unit tested.
 *
 * @module
 */

/** What Chromium reports about the connection and device, where it does. */
export interface NetworkHints {
	readonly connection?: {
		readonly saveData?: boolean;
		readonly effectiveType?: string;
	};
	readonly deviceMemory?: number;
}

/**
 * Whether Rive is worth its download here. It is about 300 KB on the wire
 * (Brotli), over three times the rest of the homepage, for an animation the CSS stroke draws
 * nearly as well: skip it when the reader asked to save data, when the link
 * is slower than 4G, or on a device with under 4 GB of memory, where
 * compiling its WebAssembly costs more than the flourish is worth.
 */
export function riveWorthLoading(hints: NetworkHints): boolean {
	const connection = hints.connection;
	if (connection?.saveData) return false;
	if (connection?.effectiveType && connection.effectiveType !== "4g") {
		return false;
	}
	if (hints.deviceMemory !== undefined && hints.deviceMemory < 4) return false;
	return true;
}
