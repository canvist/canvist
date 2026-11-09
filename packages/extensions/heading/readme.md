# @canvist/extension-heading

TipTap's `heading` block, with `#` to `######` and `Mod-Alt-1` to `Mod-Alt-6`, for [canvist](https://github.com/canvist/canvist).

This package exports `heading` from `@canvist/core/extensions`, so you can import it on its own, as TipTap ships each extension. It loads no WebAssembly: an engine package brings the module, and every one of them, from the StarterKit module `@canvist/headless` up, has these extensions.

```sh
npm install @canvist/headless @canvist/extension-heading
```

```ts
import { heading } from "@canvist/extension-heading";
import { createEngine } from "@canvist/headless";

const engine = await createEngine([heading]);
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.

## License

[Unlicense](https://unlicense.org)
