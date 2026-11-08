# canvist_extension_subscript

TipTap's `subscript` mark, with `Mod-,`, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `subscript` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_subscript/canvist_extension_subscript.dart';

final builder = EngineBuilder();
final subscriptHandle = builder.add(subscript);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
