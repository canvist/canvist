/**
 * Subsets the variable fonts to the characters and the weight range the site
 * uses, so the font on the critical path is as small as it can be. Narrowing
 * an axis drops the variation data for the weights outside it.
 *
 *   deno run --allow-read --allow-write --allow-env scripts/subset-fonts.ts
 *
 * Re-run when copy needs a character outside CHARACTERS; the output is
 * committed in src/fonts/.
 */
import { Buffer } from "node:buffer";
import subsetFont from "subset-font";

const ASCII = Array.from(
	{ length: 95 },
	(_, index) => String.fromCharCode(32 + index),
).join("");
/** Typography the copy uses beyond ASCII. */
const EXTRA = "–—‘’“”…·•µ×éè";
const CHARACTERS = ASCII + EXTRA;

const FONTS = [
	{
		from:
			"node_modules/@fontsource-variable/bricolage-grotesque/files/bricolage-grotesque-latin-opsz-normal.woff2",
		to: "src/fonts/bricolage-grotesque-subset.woff2",
		// The lightest weight the site sets is 400 (`grep -r font-weight src`).
		axes: { wght: { min: 400, max: 800 } },
	},
	{
		from:
			"node_modules/@fontsource-variable/geist-mono/files/geist-mono-latin-wght-normal.woff2",
		to: "src/fonts/geist-mono-subset.woff2",
		// Code and measurements use 400 to 650.
		axes: { wght: { min: 400, max: 700 } },
	},
];

const root = new URL("..", import.meta.url);

for (const font of FONTS) {
	// subset-font sniffs the format with Buffer methods, so hand it a Buffer.
	const source = Buffer.from(await Deno.readFile(new URL(font.from, root)));
	const subset = await subsetFont(source, CHARACTERS, {
		targetFormat: "woff2",
		variationAxes: font.axes,
	});
	await Deno.writeFile(new URL(font.to, root), subset);
	console.log(`${font.to}: ${source.byteLength} → ${subset.byteLength} bytes`);
}
