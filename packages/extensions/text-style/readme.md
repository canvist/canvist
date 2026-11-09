# @canvist/extension-text-style

TipTap's `textStyle` mark and the styles on it: colour, background colour, font family, font size, and line height, for [canvist](https://github.com/canvist/canvist).

This package exports `textStyle`, `color`, `backgroundColor`, `fontFamily`, `fontSize`, `lineHeight` from `@canvist/headless/extras`, so you can import them on their own, as TipTap ships each extension. canvist compiles every extension into its WebAssembly module, so this package adds no code: these extensions are in the full module, `@canvist/headless/full`, so create the engine from there.

```sh
npm install @canvist/headless @canvist/extension-text-style
```

```ts
import { textStyle } from "@canvist/extension-text-style";
import { createEngine } from "@canvist/headless/full";

const engine = await createEngine([textStyle]);
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.

## License

[Unlicense](https://unlicense.org)
