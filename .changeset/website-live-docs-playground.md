---
website: feat
---

# Run the docs' samples on the page, and add a playground

- Every runnable TypeScript sample in the docs, and the browser samples marked `live` (`ts no_run live`), gets a **Run** button. It turns the sample into an editor that reruns as it changes, shows what it printed (and whether that matches the comments `docs/check-snippets.ts` checks), and previews the document it made: painted by `@canvist/web` when it only uses the StarterKit, and as HTML, JSON, and Markdown.
- `/playground` runs examples with server code and browser code: the editor with a toolbar built from its extensions, a document using all 49 extensions, a collaboration server with live peers, and a `node:http` server. A switch for every extension rewrites the code's extension list and import.
- The sandbox (`website/src/sandbox`) runs server code in a worker with `@canvist/headless` (and `/full`, `/ssr`), `ws`, `node:http`, `node:events`, `Deno.serve`, `process`, and `Buffer`, on a `localhost` that only the tab can reach. Browser code runs in frames whose `WebSocket` and `fetch` to `localhost` reach that worker. Code only comes from the page itself, never a URL.
- Agents script the playground through `window.canvistPlayground`, documented in the agents guide.
