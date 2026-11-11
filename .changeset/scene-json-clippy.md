---
"canvist_scene": fix
---

# Pass scene JSON paths by reference while flattening

`flatten` takes `&Arc<str>` for the path prefix instead of cloning the `Arc` into the parameter, so clippy's `needless_pass_by_value` lint no longer fails the browser and desktop builds.
