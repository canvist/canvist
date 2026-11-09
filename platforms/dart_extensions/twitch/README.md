# canvist_extension_twitch

TipTap's `twitch` block, embedding only Twitch URLs, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `twitch` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_twitch/canvist_extension_twitch.dart';

final builder = EngineBuilder();
final twitchHandle = builder.add(twitch);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
