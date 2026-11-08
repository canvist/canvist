# canvist_extension_italic

TipTap's `italic` mark, with `Mod-i`, `*text*`, and `_text_`, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `italic` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_italic/canvist_extension_italic.dart';

final builder = EngineBuilder();
final italicHandle = builder.add(italic);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
