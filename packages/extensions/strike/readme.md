# @canvist/extension-strike

TipTap's `strike` mark, with `Mod-Shift-s` and `~~text~~`, for [canvist](https://github.com/canvist/canvist).

This package exports `strike` from `@canvist/core/extensions`, so you can import it on its own, as TipTap ships each extension. It loads no WebAssembly: an engine package brings the module, and every one of them, from the StarterKit module `@canvist/headless` up, has these extensions.

```sh
npm install @canvist/headless @canvist/extension-strike
```

```ts
import { strike } from "@canvist/extension-strike";
import { createEngine } from "@canvist/headless";

const engine = await createEngine([strike]);
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.

## License

[Unlicense](https://unlicense.org)
