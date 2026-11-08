# canvist_extension_callout

Remirror's `callout` block: a tinted box with a type, made by typing `:::warning`, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `callout` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_callout/canvist_extension_callout.dart';

final builder = EngineBuilder();
final calloutHandle = builder.add(callout);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
