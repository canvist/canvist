# canvist_extensions

All built-in canvist extensions and their typed command handles, in one Dart package.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extensions/canvist_extensions.dart' as extensions;

final builder = EngineBuilder();
final bold = builder.add(extensions.bold);
final table = builder.add(extensions.table);
final engine = await builder.build();
```

The package exports every extension descriptor and handle class, as well as convenient constants such as `bold`, `table`, and `fontSize`. It re-exports the same types from `canvist`; existing code using `Extensions.bold` continues to work. The engine compiles the native extensions, so this package adds no native library or build hook.

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists commands, queries, and key bindings.

`node tools/extension-packages.ts` generates the library and manifest alongside the npm extension packages. Dart keeps the extensions together, so only one pub.dev name and trusted publisher are needed.
