# canvist_extension_code_block

TipTap's `codeBlock` block, with fenced ``` and `Mod-Alt-c`, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `codeBlock` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_code_block/canvist_extension_code_block.dart';

final builder = EngineBuilder();
final codeBlockHandle = builder.add(codeBlock);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
