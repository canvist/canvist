---
"canvist_collab": breaking
"@canvist/headless": patch
---

# Bound collaborative block tree nesting

Reject collaborative block trees deeper than 64 levels before recursive mirroring can exhaust the native or WebAssembly stack. The limit counts existing ancestors during incremental updates and also covers full reads and hidden subtrees. Rejection leaves the model unchanged; after a correcting update the binding rebuilds from the bounded shared state.

`canvist_collab` adds `MAX_BLOCK_DEPTH` and `CollabError::BlockDepthLimit`. Exhaustive matches on `CollabError` must handle the new variant. JavaScript callers receive the existing `collab` error code.

```rust
use canvist_collab::CollabError;
use canvist_collab::MAX_BLOCK_DEPTH;

assert_eq!(
	CollabError::BlockDepthLimit.to_string(),
	format!("shared block nesting exceeds the limit of {MAX_BLOCK_DEPTH} levels"),
);
```
