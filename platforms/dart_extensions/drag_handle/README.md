# canvist_extension_drag_handle

Move blocks up and down with `Alt-↑` and `Alt-↓`, or to any position, like TipTap's DragHandle, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `dragHandle` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_drag_handle/canvist_extension_drag_handle.dart';

final builder = EngineBuilder();
final dragHandleHandle = builder.add(dragHandle);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
