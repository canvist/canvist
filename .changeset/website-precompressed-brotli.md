---
"website": perf
---

# Serve the website precompressed with Brotli

The website build writes a Brotli (quality 11) and a gzip copy beside every compressible file, and Caddy serves those copies instead of compressing each response with zstd on the fly. Every script, stylesheet, page, and WebAssembly module is about 20% smaller on the wire: the homepage goes from 490 KiB to 394 KiB, and the playground's editor module from 351 KiB to 277 KiB.

`deno task perf:serve` and `deno task perf:pages` measure the bytes, paint timings, blocking time, and editor latency of each page; `docs/internal/web-performance.md` explains them and logs the results.
