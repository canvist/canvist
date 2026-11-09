# canvist_extension_comments

A `comment` mark holding thread ids, for comment threads anchored to text, for [canvist](https://github.com/canvist/canvist)'s Dart engine.

This package exports `comments` from `package:canvist`, so you can import it on its own. canvist compiles every extension into its engine, so this package adds no code.

```dart
import 'package:canvist/canvist.dart';
import 'package:canvist_extension_comments/canvist_extension_comments.dart';

final builder = EngineBuilder();
final commentsHandle = builder.add(comments);
final engine = await builder.build();
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.
