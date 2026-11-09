# canvist_extension_find_and_replace

Search a canvist document, step through matches, and replace them, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `findAndReplace` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_find_and_replace/canvist_extension_find_and_replace.dart';

final builder = EngineBuilder();
final findAndReplaceHandle = builder.add(findAndReplace);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
