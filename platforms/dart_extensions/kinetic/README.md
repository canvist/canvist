# canvist_extension_kinetic

A `kinetic` mark for text that moves: wave, shake, pulse, and more, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `kinetic` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_kinetic/canvist_extension_kinetic.dart';

final builder = EngineBuilder();
final kineticHandle = builder.add(kinetic);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
