# canvist_extension_hashtag

Remirror's `hashtag` mark, applied to `#tag` as you type, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `hashtag` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_hashtag/canvist_extension_hashtag.dart';

final builder = EngineBuilder();
final hashtagHandle = builder.add(hashtag);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
