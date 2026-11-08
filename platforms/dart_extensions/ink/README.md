# canvist_extension_ink

An `ink` block holding freehand strokes that the canvist canvas draws, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `ink` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_ink/canvist_extension_ink.dart';

final builder = EngineBuilder();
final inkHandle = builder.add(ink);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
