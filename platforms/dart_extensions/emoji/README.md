# canvist_extension_emoji

TipTap's `emoji` node, with `:smile:` shortcodes and `:` suggestions from GitHub's gemoji, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `emoji` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_emoji/canvist_extension_emoji.dart';

final builder = EngineBuilder();
final emojiHandle = builder.add(emoji);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
