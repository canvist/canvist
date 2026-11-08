# @canvist/extension-trailing-node

Keeps a paragraph at the end of the document, like TipTap's `TrailingNode`, for [canvist](https://github.com/canvist/canvist).

This package exports `trailingNode` from `@canvist/headless/extensions`, so you can import it on its own, as TipTap ships each extension. canvist compiles every extension into its WebAssembly module, so this package adds no code: the StarterKit module, `@canvist/headless`, has these extensions.

```sh
npm install @canvist/headless @canvist/extension-trailing-node
```

```ts
import { trailingNode } from "@canvist/extension-trailing-node";
import { createEngine } from "@canvist/headless";

const engine = await createEngine([trailingNode]);
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.

## License

[Unlicense](https://unlicense.org)
