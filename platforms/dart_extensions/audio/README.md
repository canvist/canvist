# canvist_extension_audio

TipTap's `audio` block: an audio player, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `audio` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_audio/canvist_extension_audio.dart';

final builder = EngineBuilder();
final audioHandle = builder.add(audio);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
