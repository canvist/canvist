# @canvist/extension-mathematics

TipTap's `inlineMath` and `blockMath` nodes, holding LaTeX typed as `$$x$$`, for [canvist](https://github.com/canvist/canvist).

This package exports `mathematics` from `@canvist/headless/extras`, so you can import it on its own, as TipTap ships each extension. canvist compiles every extension into its WebAssembly module, so this package adds no code: these extensions are in the full module, `@canvist/headless/full`, so create the engine from there.

```sh
npm install @canvist/headless @canvist/extension-mathematics
```

```ts
import { mathematics } from "@canvist/extension-mathematics";
import { createEngine } from "@canvist/headless/full";

const engine = await createEngine([mathematics]);
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.

## License

[Unlicense](https://unlicense.org)
