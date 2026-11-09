# @canvist/extension-blockquote

TipTap's `blockquote` block, with a leading `>` and `Mod-Shift-b`, for [canvist](https://github.com/canvist/canvist).

This package exports `blockquote` from `@canvist/headless/extensions`, so you can import it on its own, as TipTap ships each extension. canvist compiles every extension into its WebAssembly module, so this package adds no code: the StarterKit module, `@canvist/headless`, has these extensions.

```sh
npm install @canvist/headless @canvist/extension-blockquote
```

```ts
import { blockquote } from "@canvist/extension-blockquote";
import { createEngine } from "@canvist/headless";

const engine = await createEngine([blockquote]);
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.

## License

[Unlicense](https://unlicense.org)
