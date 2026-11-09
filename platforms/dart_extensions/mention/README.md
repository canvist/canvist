# canvist_extension_mention

TipTap's `mention` node, with `@` suggestions, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `mention` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_mention/canvist_extension_mention.dart';

final builder = EngineBuilder();
final mentionHandle = builder.add(mention);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
