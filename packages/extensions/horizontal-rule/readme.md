# @canvist/extension-horizontal-rule

TipTap's `horizontalRule` block, with `---`, for [canvist](https://github.com/canvist/canvist).

This package exports `horizontalRule` from `@canvist/core/extensions`, so you can import it on its own, as TipTap ships each extension. It loads no WebAssembly: an engine package brings the module, and every one of them, from the StarterKit module `@canvist/headless` up, has these extensions.

```sh
npm install @canvist/headless @canvist/extension-horizontal-rule
```

```ts
import { horizontalRule } from "@canvist/extension-horizontal-rule";
import { createEngine } from "@canvist/headless";

const engine = await createEngine([horizontalRule]);
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.

## License

[Unlicense](https://unlicense.org)
