# @canvist/extension-hashtag

Remirror's `hashtag` mark, applied to `#tag` as you type, for [canvist](https://github.com/canvist/canvist).

This package exports `hashtag` from `@canvist/core/extras`, so you can import it on its own, as TipTap ships each extension. It loads no WebAssembly: an engine package brings the module, and these extensions are in the full modules, `@canvist/headless/full` and `@canvist/collab/full`, so create the engine from one of those.

```sh
npm install @canvist/headless @canvist/extension-hashtag
```

```ts
import { hashtag } from "@canvist/extension-hashtag";
import { createEngine } from "@canvist/headless/full";

const engine = await createEngine([hashtag]);
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.

## License

[Unlicense](https://unlicense.org)
