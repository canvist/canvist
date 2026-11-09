# canvist_extension_list

TipTap's lists: `bulletList`, `orderedList`, `listItem`, `taskList`, and `taskItem`, with Markdown shortcuts and Tab to nest, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `bulletList`, `orderedList`, `listItem`, `taskList` from `package:canvist`, so you can import them on their own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_list/canvist_extension_list.dart';

final builder = EngineBuilder();
final bulletListHandle = builder.add(bulletList);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
