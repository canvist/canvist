# @canvist/extension-list

TipTap's lists: `bulletList`, `orderedList`, `listItem`, `taskList`, and `taskItem`, with Markdown shortcuts and Tab to nest, for [canvist](https://github.com/canvist/canvist).

This package exports `bulletList`, `orderedList`, `listItem`, `taskList` from `@canvist/headless/extensions`, so you can import them on their own, as TipTap ships each extension. canvist compiles every extension into its WebAssembly module, so this package adds no code: the StarterKit module, `@canvist/headless`, has these extensions.

```sh
npm install @canvist/headless @canvist/extension-list
```

```ts
import { bulletList } from "@canvist/extension-list";
import { createEngine } from "@canvist/headless";

const engine = await createEngine([bulletList]);
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.

## License

[Unlicense](https://unlicense.org)
