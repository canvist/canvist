# @canvist/extension-code-block

TipTap's `codeBlock` block, with fenced ``` and `Mod-Alt-c`, for [canvist](https://github.com/canvist/canvist).

This package exports `codeBlock` from `@canvist/core/extensions`, so you can import it on its own, as TipTap ships each extension. It loads no WebAssembly: an engine package brings the module, and every one of them, from the StarterKit module `@canvist/headless` up, has these extensions.

```sh
npm install @canvist/headless @canvist/extension-code-block
```

```ts
import { codeBlock } from "@canvist/extension-code-block";
import { createEngine } from "@canvist/headless";

const engine = await createEngine([codeBlock]);
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.

## License

[Unlicense](https://unlicense.org)
