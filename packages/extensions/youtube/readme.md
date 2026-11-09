# @canvist/extension-youtube

TipTap's `youtube` block, embedding only YouTube URLs, for [canvist](https://github.com/canvist/canvist).

This package exports `youtube` from `@canvist/headless/extras`, so you can import it on its own, as TipTap ships each extension. canvist compiles every extension into its WebAssembly module, so this package adds no code: these extensions are in the full module, `@canvist/headless/full`, so create the engine from there.

```sh
npm install @canvist/headless @canvist/extension-youtube
```

```ts
import { youtube } from "@canvist/extension-youtube";
import { createEngine } from "@canvist/headless/full";

const engine = await createEngine([youtube]);
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.

## License

[Unlicense](https://unlicense.org)
