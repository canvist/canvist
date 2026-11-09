# @canvist/extension-bold

TipTap's `bold` mark, with `Mod-b`, `**text**`, and `__text__`, for [canvist](https://github.com/canvist/canvist).

This package exports `bold` from `@canvist/headless/extensions`, so you can import it on its own, as TipTap ships each extension. canvist compiles every extension into its WebAssembly module, so this package adds no code: the StarterKit module, `@canvist/headless`, has these extensions.

```sh
npm install @canvist/headless @canvist/extension-bold
```

```ts
import { bold } from "@canvist/extension-bold";
import { createEngine } from "@canvist/headless";

const engine = await createEngine([bold]);
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.

## License

[Unlicense](https://unlicense.org)
