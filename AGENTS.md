# Package IDs across ecosystems

Give every Rust crate the monochange package ID `rust:` followed by its crate name, for example `rust:canvist_core` or `rust:canvist_extension_table`. Give Dart packages that share a name with a crate the ID `dart:` followed by the package name, for example Rust `rust:canvist` and Dart `dart:canvist`, or Rust `rust:canvist_extensions` and Dart `dart:canvist_extensions`.

Keep npm's scoped package names, such as `@canvist/core`, unchanged; their scopes already distinguish them. Do not add an `npm:` prefix to these IDs.

Use the same IDs in package declarations, group membership, changesets, and release-tool references, and change them together. Do not change published package names to resolve an internal ID collision. Validate both `monochange check` and `monochange preview` after changing IDs.
