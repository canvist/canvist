# @canvist/extension-ink

An `ink` block holding freehand strokes that the canvist canvas draws, for [canvist](https://github.com/canvist/canvist).

This package exports `ink` from `@canvist/core/extras`, so you can import it on its own, as TipTap ships each extension. It loads no WebAssembly: an engine package brings the module, and these extensions are in the full modules, `@canvist/headless/full` and `@canvist/collab/full`, so create the engine from one of those.

```sh
npm install @canvist/headless @canvist/extension-ink
```

```ts
import { ink } from "@canvist/extension-ink";
import { createEngine } from "@canvist/headless/full";

const engine = await createEngine([ink]);
```

The [extension catalogue](https://github.com/canvist/canvist/blob/main/docs/src/extensions/readme.md) lists each extension's commands, queries, and keys.

## License

[Unlicense](https://unlicense.org)
