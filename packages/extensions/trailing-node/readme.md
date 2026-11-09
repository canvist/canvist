# @canvist/extension-trailing-node

Keeps a paragraph at the end of the document, like TipTap's `TrailingNode`, for [canvist](https://github.com/canvist/canvist).

This package exports `trailingNode` from `@canvist/core/extensions`, so you can import it on its own, as TipTap ships each extension. It loads no WebAssembly: an engine package brings the module, and every one of them, from the StarterKit module `@canvist/headless` up, has these extensions.

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
