# canvist_extension_placeholder

Hint text in empty blocks, like TipTap's Placeholder, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `placeholder` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_placeholder/canvist_extension_placeholder.dart';

final builder = EngineBuilder();
final placeholderHandle = builder.add(placeholder);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
