# canvist_extension_typography

Smart quotes, dashes, arrows, ellipses, and fractions as you type, like TipTap's Typography, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `typography` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_typography/canvist_extension_typography.dart';

final builder = EngineBuilder();
final typographyHandle = builder.add(typography);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
