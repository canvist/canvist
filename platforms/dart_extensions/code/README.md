# canvist_extension_code

TipTap's inline `code` mark, with `Mod-e` and `` `text` ``, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `code` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_code/canvist_extension_code.dart';

final builder = EngineBuilder();
final codeHandle = builder.add(code);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
