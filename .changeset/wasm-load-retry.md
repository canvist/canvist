---
"@canvist/core": fix
"@canvist/headless": fix
"@canvist/collab": fix
"@canvist/ssr": fix
"@canvist/scene": fix
"@canvist/web": fix
---

# A WebAssembly module that failed to load can load again

Each package loaded its module once and kept the first promise, failure included. After a dropped connection, or a first call without the module a Worker has to pass, every later `createEngine`, `createEditor`, `createRenderer`, and `createSceneEngine` failed the same way until the page or process restarted. A failed load is now forgotten, so the next call tries again with its own source. `@canvist/core` has the shared `loadOnce` the packages use.
