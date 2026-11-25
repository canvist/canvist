---
"rust:canvist_cloud": perf
---

# Compress Convert and document export responses

`POST /v1/convert`, `GET /v1/documents`, and `GET /v1/documents/{name}` now answer with Brotli or gzip when the client's `Accept-Encoding` allows it, at level 4. A converted or exported document is HTML, Markdown, or JSON, so it arrives about two thirds smaller: an 18.5 KB Markdown export is 6.3 KB with Brotli, for about a millisecond of server CPU. WebSocket routes are unchanged, and clients that send no `Accept-Encoding` get the same bytes as before.
