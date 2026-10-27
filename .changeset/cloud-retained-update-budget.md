---
"canvist_cloud": fix
---

# Bound updates waiting for missing dependencies

Count retained Yjs submissions toward the document byte limit, including updates pending inside yrs, and cap unresolved batches at 128 distinct submissions per room. Identical replays are deduplicated before decoding and readiness scans. Release the retained-input charges when dependencies resolve and both buffers drain.
