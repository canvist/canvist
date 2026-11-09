# @canvist/extension-highlight

TipTap's `highlight` mark, with colours, `==text==`, and `Mod-Shift-h`, for [canvist](https://github.com/canvist/canvist).

This package exports `highlight` from `@canvist/headless/extras`, so you can import it on its own, as TipTap ships each extension. canvist compiles every extension into its WebAssembly module, so this package adds no code: these extensions are in the full module, `@canvist/headless/full`, so create the engine from there.

```sh
npm install @canvist/headless @canvist/extension-highlight
```

```ts
import { highlight } from "@canvist/extension-highlight";
import { createEngine } from "@canvist/headless/full";

const engine = await createEngine([highlight]);
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.

## License

[Unlicense](https://unlicense.org)
