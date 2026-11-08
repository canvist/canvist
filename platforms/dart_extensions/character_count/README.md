# canvist_extension_character_count

Character and word counts with an optional limit, like TipTap's CharacterCount, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `characterCount` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_character_count/canvist_extension_character_count.dart';

final builder = EngineBuilder();
final characterCountHandle = builder.add(characterCount);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
