# canvist_extension_clear_formatting

Remove marks and turn blocks back into paragraphs, with `Mod-\`, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `clearFormatting` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_clear_formatting/canvist_extension_clear_formatting.dart';

final builder = EngineBuilder();
final clearFormattingHandle = builder.add(clearFormatting);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
