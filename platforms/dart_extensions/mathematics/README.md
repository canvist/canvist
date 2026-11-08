# canvist_extension_mathematics

TipTap's `inlineMath` and `blockMath` nodes, holding LaTeX typed as `$$x$$`, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `mathematics` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_mathematics/canvist_extension_mathematics.dart';

final builder = EngineBuilder();
final mathematicsHandle = builder.add(mathematics);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
