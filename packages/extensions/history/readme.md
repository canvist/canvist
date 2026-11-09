# @canvist/extension-history

Undo and redo keys for the canvist editor: `Mod-z`, `Mod-Shift-z`, and `Mod-y`, for [canvist](https://github.com/canvist/canvist).

This package exports `history` from `@canvist/headless/extensions`, so you can import it on its own, as TipTap ships each extension. canvist compiles every extension into its WebAssembly module, so this package adds no code: the StarterKit module, `@canvist/headless`, has these extensions.

```sh
npm install @canvist/headless @canvist/extension-history
```

```ts
import { history } from "@canvist/extension-history";
import { createEngine } from "@canvist/headless";

const engine = await createEngine([history]);
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.

## License

[Unlicense](https://unlicense.org)
