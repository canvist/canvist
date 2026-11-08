# canvist_extension_hard_break

TipTap's `hardBreak` line break, with `Shift-Enter`, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `hardBreak` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_hard_break/canvist_extension_hard_break.dart';

final builder = EngineBuilder();
final hardBreakHandle = builder.add(hardBreak);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
