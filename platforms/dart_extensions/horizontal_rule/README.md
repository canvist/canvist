# canvist_extension_horizontal_rule

TipTap's `horizontalRule` block, with `---`, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `horizontalRule` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_horizontal_rule/canvist_extension_horizontal_rule.dart';

final builder = EngineBuilder();
final horizontalRuleHandle = builder.add(horizontalRule);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
