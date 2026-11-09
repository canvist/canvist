# @canvist/extension-comments

A `comment` mark holding thread ids, for comment threads anchored to text, for [canvist](https://github.com/canvist/canvist).

This package exports `comments` from `@canvist/core/extras`, so you can import it on its own, as TipTap ships each extension. It loads no WebAssembly: an engine package brings the module, and these extensions are in the full modules, `@canvist/headless/full` and `@canvist/collab/full`, so create the engine from one of those.

```sh
npm install @canvist/headless @canvist/extension-comments
```

```ts
import { comments } from "@canvist/extension-comments";
import { createEngine } from "@canvist/headless/full";

const engine = await createEngine([comments]);
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.

## License

[Unlicense](https://unlicense.org)
