# canvist_extension_link

TipTap's `link` mark, with autolinking and linking on paste, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `link` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_link/canvist_extension_link.dart';

final builder = EngineBuilder();
final linkHandle = builder.add(link);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
