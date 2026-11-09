# canvist_extension_table

TipTap's `table`, `tableRow`, `tableCell`, and `tableHeader`, with commands to add, remove, merge, and split cells, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `table` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_table/canvist_extension_table.dart';

final builder = EngineBuilder();
final tableHandle = builder.add(table);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
