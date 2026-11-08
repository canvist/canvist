# canvist_extension_history

Undo and redo keys for the canvist editor: `Mod-z`, `Mod-Shift-z`, and `Mod-y`, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `history` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_history/canvist_extension_history.dart';

final builder = EngineBuilder();
final historyHandle = builder.add(history);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
