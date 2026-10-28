/**
 * The output a sample promises in its comments: the comment lines straight
 * after each line that prints. It's the rule `docs/check-snippets.ts`
 * checks samples with under Deno (against the output with trailing
 * whitespace trimmed), so the page can say whether a run
 * printed what the docs say.
 */
export function expectedOutput(code: string): string[] | undefined {
	const lines = code.split("\n");
	const expected: string[] = [];
	let promised = false;
	for (let index = 0; index < lines.length; index++) {
		if (!/^\s*console\.log\(/.test(lines[index] ?? "")) continue;
		for (let next = index + 1; next < lines.length; next++) {
			const comment = /^\s*\/\/ ?(.*)$/.exec(lines[next] ?? "");
			if (!comment) break;
			expected.push(comment[1] ?? "");
			promised = true;
		}
	}
	return promised ? expected : undefined;
}
