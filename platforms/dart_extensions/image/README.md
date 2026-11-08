# canvist_extension_image

TipTap's `image` block, with `setImage` and `![alt](src)`, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `image` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_image/canvist_extension_image.dart';

final builder = EngineBuilder();
final imageHandle = builder.add(image);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
