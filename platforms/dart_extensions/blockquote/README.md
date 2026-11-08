# canvist_extension_blockquote

TipTap's `blockquote` block, with a leading `>` and `Mod-Shift-b`, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `blockquote` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_blockquote/canvist_extension_blockquote.dart';

final builder = EngineBuilder();
final blockquoteHandle = builder.add(blockquote);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
