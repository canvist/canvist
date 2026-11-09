# canvist_extension_highlight

TipTap's `highlight` mark, with colours, `==text==`, and `Mod-Shift-h`, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `highlight` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_highlight/canvist_extension_highlight.dart';

final builder = EngineBuilder();
final highlightHandle = builder.add(highlight);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
