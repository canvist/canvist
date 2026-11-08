# @canvist/extension-link

TipTap's `link` mark, with autolinking and linking on paste, for [canvist](https://github.com/canvist/canvist).

This package exports `link` from `@canvist/headless/extensions`, so you can import it on its own, as TipTap ships each extension. canvist compiles every extension into its WebAssembly module, so this package adds no code: the StarterKit module, `@canvist/headless`, has these extensions.

```sh
npm install @canvist/headless @canvist/extension-link
```

```ts
import { link } from "@canvist/extension-link";
import { createEngine } from "@canvist/headless";

const engine = await createEngine([link]);
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.

## License

[Unlicense](https://unlicense.org)
