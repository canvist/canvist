# canvist_extension_table_of_contents

A canvist document's headings as an outline, like TipTap's TableOfContents, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `tableOfContents` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_table_of_contents/canvist_extension_table_of_contents.dart';

final builder = EngineBuilder();
final tableOfContentsHandle = builder.add(tableOfContents);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
