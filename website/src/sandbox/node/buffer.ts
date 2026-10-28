/**
 * The part of Node's `Buffer` server code reaches for: bytes that print as
 * text with `toString()`, as `ws` messages and request bodies do in Node.
 */

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export type BufferEncoding = "utf8" | "utf-8" | "base64" | "hex";

/** The bytes; the static helpers Node has are added to it below. */
class Bytes extends Uint8Array {
	override toString(encoding: BufferEncoding = "utf8"): string {
		if (encoding === "base64") {
			return btoa(
				Array.from(this, (byte) => String.fromCharCode(byte)).join(""),
			);
		}
		if (encoding === "hex") {
			return Array.from(this, (byte) => byte.toString(16).padStart(2, "0"))
				.join("");
		}
		return decoder.decode(this);
	}

	toJSON(): { type: "Buffer"; data: number[] } {
		return { type: "Buffer", data: Array.from(this) };
	}
}

function from(
	value: string | ArrayBuffer | ArrayLike<number>,
	encoding: BufferEncoding = "utf8",
): Bytes {
	if (typeof value !== "string") {
		return value instanceof ArrayBuffer
			? new Bytes(value)
			: new Bytes(Array.from(value));
	}
	if (encoding === "base64") {
		return new Bytes(Array.from(atob(value), (char) => char.charCodeAt(0)));
	}
	if (encoding === "hex") {
		return new Bytes(
			(value.match(/../g) ?? []).map((pair) => Number.parseInt(pair, 16)),
		);
	}
	return new Bytes(encoder.encode(value));
}

/** Bytes of a frame or chunk, as a `Buffer`. */
export function bufferOf(data: string | ArrayBuffer | Uint8Array): Bytes {
	if (typeof data === "string") return from(data);
	if (data instanceof ArrayBuffer) return new Bytes(data);
	const bytes = new Bytes(data.byteLength);
	bytes.set(data);
	return bytes;
}

export const Buffer = Object.assign(Bytes, {
	from,
	alloc: (size: number) => new Bytes(size),
	isBuffer: (value: unknown): value is Bytes => value instanceof Bytes,
	byteLength: (value: string) => encoder.encode(value).byteLength,
	concat(chunks: readonly Uint8Array[]): Bytes {
		const joined = new Bytes(
			chunks.reduce((length, chunk) => length + chunk.byteLength, 0),
		);
		let offset = 0;
		for (const chunk of chunks) {
			joined.set(chunk, offset);
			offset += chunk.byteLength;
		}
		return joined;
	},
});
