# canvist_extension_text_align

TipTap's `textAlign` attribute, with `Mod-Shift-l`, `e`, `r`, and `j`, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `textAlign` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_text_align/canvist_extension_text_align.dart';

final builder = EngineBuilder();
final textAlignHandle = builder.add(textAlign);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
