# @canvist/extension-conditional

A `conditional` mark for text kept only in some versions of a document, for [canvist](https://github.com/canvist/canvist).

This package exports `conditional` from `@canvist/headless/extras`, so you can import it on its own, as TipTap ships each extension. canvist compiles every extension into its WebAssembly module, so this package adds no code: these extensions are in the full module, `@canvist/headless/full`, so create the engine from there.

```sh
npm install @canvist/headless @canvist/extension-conditional
```

```ts
import { conditional } from "@canvist/extension-conditional";
import { createEngine } from "@canvist/headless/full";

const engine = await createEngine([conditional]);
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.

## License

[Unlicense](https://unlicense.org)
