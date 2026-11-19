---
"@canvist/canvas": fix
---

# Load the WebAssembly module again after a failed load

`mountCanvas` kept the promise of its first module load even when that load failed, so one dropped download made every later `mountCanvas` reject until the page reloaded. A failed load is now forgotten, and the next mount fetches the module again.
