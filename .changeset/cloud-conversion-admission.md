---
"canvist_cloud": fix
---

# Bound conversion admission before reading input

Reserve conversion capacity before reading JSON request bodies or loading stored documents for export. When every slot is occupied, return `503` with code `converter_unavailable` immediately instead of retaining inputs in an unbounded semaphore queue. Cancelled uploads and rejected inputs release their slots, while timed-out blocking work retains capacity until it finishes.
