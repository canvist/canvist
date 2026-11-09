# canvist_extension_strike

TipTap's `strike` mark, with `Mod-Shift-s` and `~~text~~`, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `strike` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_strike/canvist_extension_strike.dart';

final builder = EngineBuilder();
final strikeHandle = builder.add(strike);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
