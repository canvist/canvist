# @canvist/extension-table

TipTap's `table`, `tableRow`, `tableCell`, and `tableHeader`, with commands to add, remove, merge, and split cells, for [canvist](https://github.com/canvist/canvist).

This package exports `table` from `@canvist/headless/extras`, so you can import it on its own, as TipTap ships each extension. canvist compiles every extension into its WebAssembly module, so this package adds no code: these extensions are in the full module, `@canvist/headless/full`, so create the engine from there.

```sh
npm install @canvist/headless @canvist/extension-table
```

```ts
import { table } from "@canvist/extension-table";
import { createEngine } from "@canvist/headless/full";

const engine = await createEngine([table]);
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.

## License

[Unlicense](https://unlicense.org)
