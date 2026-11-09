# canvist_extension_text_style

TipTap's `textStyle` mark and the styles on it: colour, background colour, font family, font size, and line height, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `textStyle`, `color`, `backgroundColor`, `fontFamily`, `fontSize`, `lineHeight` from `package:canvist`, so you can import them on their own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_text_style/canvist_extension_text_style.dart';

final builder = EngineBuilder();
final textStyleHandle = builder.add(textStyle);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
