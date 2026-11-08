# canvist_extension_file

Remirror's `file` block: an attachment card with a name, type, and size, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `file` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_file/canvist_extension_file.dart';

final builder = EngineBuilder();
final fileHandle = builder.add(file);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
