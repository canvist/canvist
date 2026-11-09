# canvist_extension_underline

TipTap's `underline` mark, with `Mod-u`, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `underline` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_underline/canvist_extension_underline.dart';

final builder = EngineBuilder();
final underlineHandle = builder.add(underline);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
