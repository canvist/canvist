# canvist_extension_details

TipTap's `details` block: a summary that opens to show its content, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `details` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_details/canvist_extension_details.dart';

final builder = EngineBuilder();
final detailsHandle = builder.add(details);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
