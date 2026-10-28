/**
 * Code a docs sample hands to the playground ("Open in playground").
 *
 * It travels in this tab's session storage rather than the URL, so the
 * playground only ever runs code this reader wrote or opened here: a link
 * can't carry code into it.
 */

const KEY = "canvist-playground-handoff";

export interface PlaygroundHandoff {
	readonly server?: string;
	readonly client?: string;
	readonly html?: string;
}

export function savePlaygroundHandoff(handoff: PlaygroundHandoff): void {
	try {
		sessionStorage.setItem(KEY, JSON.stringify(handoff));
	} catch {
		// Storage can be blocked; the playground then opens on its first example.
	}
}

/** The code handed over, once: reading it removes it. */
export function takePlaygroundHandoff(): PlaygroundHandoff | undefined {
	try {
		const saved = sessionStorage.getItem(KEY);
		sessionStorage.removeItem(KEY);
		if (saved === null) return undefined;
		const parsed: unknown = JSON.parse(saved);
		if (typeof parsed !== "object" || parsed === null) return undefined;
		const { server, client, html } = parsed as Record<string, unknown>;
		return {
			...(typeof server === "string" ? { server } : {}),
			...(typeof client === "string" ? { client } : {}),
			...(typeof html === "string" ? { html } : {}),
		};
	} catch {
		return undefined;
	}
}
