# canvist_extension_heading

TipTap's `heading` block, with `#` to `######` and `Mod-Alt-1` to `Mod-Alt-6`, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `heading` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_heading/canvist_extension_heading.dart';

final builder = EngineBuilder();
final headingHandle = builder.add(heading);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
