# canvist_extension_trailing_node

Keeps a paragraph at the end of the document, like TipTap's `TrailingNode`, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `trailingNode` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_trailing_node/canvist_extension_trailing_node.dart';

final builder = EngineBuilder();
final trailingNodeHandle = builder.add(trailingNode);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
