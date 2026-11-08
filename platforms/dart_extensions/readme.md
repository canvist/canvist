# Dart extension packages

One package per canvist extension, such as `canvist_extension_bold` or `canvist_extension_table`, named like the Rust crates in [`crates/extensions`](../../crates/extensions). The `canvist` engine compiles every extension in, so each package only re-exports its extension from `package:canvist`, the way TipTap ships `@tiptap/extension-bold`:

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_table/canvist_extension_table.dart';

final builder = EngineBuilder();
final tableHandle = builder.add(table);
final engine = await builder.build();
```

`node tools/extension-packages.ts` writes these packages, and the npm ones in [`packages/extensions`](../../packages/extensions), from the extension metadata. Don't edit them by hand.

## Rolling them out on pub.dev

pub.dev lets one account create 4 packages every 4 hours and 12 every 24 hours, so the packages go live in batches. A package whose `pubspec.yaml` says `publish_to: none` is not live yet: the `pub` job in `publish.yml` skips it, so releases carry on meanwhile. The generator keeps that line as it finds it.

They belong to the repository's Dart workspace (the root `pubspec.yaml`), so they build against the local `canvist`.

To put the next batch live, as a pub.dev uploader of `canvist` (after `dart pub login`):

```sh
bash tools/release/reserve_dart_extensions.sh
```

It publishes a placeholder 0.0.0 for each package not yet on pub.dev, at most 4 new packages per run, and prints each one's admin page. pub.dev only changes automated publishing from a signed-in browser, so on each page turn on publishing from GitHub Actions: repository `canvist/canvist`, tag pattern `v{{version}}`, and required environment `publisher`. Then switch those packages on and commit their pubspecs:

```sh
bash tools/release/reserve_dart_extensions.sh --released canvist_extension_bold canvist_extension_table
```

Run the script again once the rate limit resets, until it reports nothing left.
