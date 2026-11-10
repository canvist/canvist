# Package IDs across ecosystems

When packages share a published name across ecosystems, keep the Rust crate's monochange package ID equal to that name, without a prefix or suffix. Qualify the other ecosystem's ID with `<ecosystem>` followed by a colon: for example, Rust `canvist_extensions` and Dart `dart:canvist_extensions`, or Rust `canvist` and Dart `dart:canvist`.

Keep npm's scoped package names, such as `@canvist/core`, unchanged; their scopes already distinguish them. Do not add an `npm:` prefix to these IDs.

Apply this convention to new name clashes and update package declarations, SDK group membership, changesets, and release-tool references together. Do not change published package names to resolve an internal ID collision. Validate both `monochange check` and `monochange preview` after changing IDs.
