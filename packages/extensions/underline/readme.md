# @canvist/extension-underline

TipTap's `underline` mark, with `Mod-u`, for [canvist](https://github.com/canvist/canvist).

This package exports `underline` from `@canvist/headless/extensions`, so you can import it on its own, as TipTap ships each extension. canvist compiles every extension into its WebAssembly module, so this package adds no code: the StarterKit module, `@canvist/headless`, has these extensions.

```sh
npm install @canvist/headless @canvist/extension-underline
```

```ts
import { underline } from "@canvist/extension-underline";
import { createEngine } from "@canvist/headless";

const engine = await createEngine([underline]);
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.

## License

[Unlicense](https://unlicense.org)
