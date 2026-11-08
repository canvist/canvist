# canvist_extension_conditional

A `conditional` mark for text kept only in some versions of a document, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `conditional` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_conditional/canvist_extension_conditional.dart';

final builder = EngineBuilder();
final conditionalHandle = builder.add(conditional);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
