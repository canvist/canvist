# Website performance

What a visit to canvist.dev costs, how to measure it, and a log of every change that moved a number. The editor's own budgets (keystroke, layout, module size) live in the user docs at [`docs/src/guides/performance.md`](../src/guides/performance.md).

## Measuring

Build the site, serve `dist/` with the production Caddyfile, and load each page in Chromium:

```sh
cd website
deno task build
deno task perf:serve &                     # caddy on PATH, port 4810
deno task perf:pages --out /tmp/head.json  # add --throttle for a mid-range phone
deno task perf:pages --compare /tmp/head.json  # after a change
```

`--url https://canvist.dev` measures what is deployed instead. `scripts/page-weight.ts` documents each metric. In short:

- **Bytes** are compressed bytes on the wire, headers included, with nothing cached, after scrolling the page to the bottom so lazy content counts.
- **FCP / LCP / TBT** are first and largest contentful paint and total blocking time, in milliseconds, the median of three loads.
- **Editor ready** is the first frame with the playground's editor laid out, from navigation. **Key → paint** is from each keydown until the editor's last canvas draw for it, which is what `editor.inputLatency()` reports; the display's next frame comes on top.
- `--throttle` is 4× CPU slowdown on a 9 Mbit/s, 40 ms link, about what Lighthouse's mobile preset emulates.

Bytes are deterministic. Timings depend on the machine, so compare runs from one machine only.

## Log

Bytes in KiB on the wire; times in ms from a 4-core Linux container, throttled.

### Precompressed Brotli (November 2026)

Caddy compressed every response on the fly with zstd at its default, fast level. The build now writes a Brotli (quality 11) and a gzip copy of every compressible file, and Caddy serves them as they are. The editor's WebAssembly goes from 351 to 277 KiB, and on-the-fly compression no longer costs the server CPU on each request.

| Page                          | Before | After        | LCP before → after (throttled) |
| ----------------------------- | -----: | ------------ | -----------------------------: |
| `/`                           |  490.5 | 394.0 (−20%) |                      340 → 344 |
| `/pricing`                    |   85.0 | 81.8 (−4%)   |                      404 → 404 |
| `/services/sync`              |   80.4 | 78.1 (−3%)   |                      252 → 248 |
| `/docs/concepts/how-it-works` |   94.5 | 89.4 (−5%)   |                      312 → 340 |
| `/playground`                 |  696.6 | 565.7 (−19%) |                      320 → 336 |

The small pages are mostly fonts: 67 KiB of the 80 on `/services/sync` is Bricolage Grotesque and Geist Mono, which are WOFF2 and gain nothing from Brotli.

### Font weight ranges (November 2026)

`scripts/subset-fonts.ts` now narrows each variable font's weight axis to the range the site uses: Bricolage Grotesque 400 to 800 (it was 200 to 800), Geist Mono 400 to 700 (100 to 900). Bricolage goes from 52.0 to 49.6 KiB and Geist Mono from 14.3 to 10.0 KiB, 6.6 KiB off every page. Screenshots of every weight the CSS sets differ only in antialiasing.
