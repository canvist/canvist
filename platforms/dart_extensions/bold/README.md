# canvist_extension_bold

TipTap's `bold` mark, with `Mod-b`, `**text**`, and `__text__`, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `bold` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_bold/canvist_extension_bold.dart';

final builder = EngineBuilder();
final boldHandle = builder.add(bold);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
