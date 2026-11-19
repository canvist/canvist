---
"@canvist/web": fix
---

# Destroying an editor stops the work it started

- An editor destroyed while fonts were loading, a plugin was setting up, or server-rendered HTML was waiting to hydrate no longer calls into its released WebAssembly afterwards: `ready()` resolves instead of throwing `null pointer passed to rust`, and fonts that finish loading later raise no unhandled rejections. A component that unmounts, or a page that navigates away, in the editor's first moments destroys it in this state.
- Faces the `fonts()` plugin finishes loading after `destroy()` no longer stay in `document.fonts`.
- A second `destroy()` does nothing instead of throwing.
- `createEditor` with content that fails to load, such as JSON with a block no registered extension defines, rejects without leaving its canvas in the element or a listener on `document.fonts`.
