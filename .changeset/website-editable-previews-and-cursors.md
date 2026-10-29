---
website: feat
---

# Type in previews, edit every extension in the playground, and see collaborators' cursors

- A docs sample's preview is the editor itself, and people can type in it: the HTML, JSON, and Markdown tabs follow their edits until the code runs again. Documents with extensions beyond the StarterKit load `@canvist/web/full`, so every sample gets an editor.
- In the playground, the Every extension document is an editor with all 49 extensions, and switching on an extension beyond the StarterKit in the Editor example moves its import to `@canvist/web/full` instead of being unavailable.
- The Collaboration example shows each peer's caret, with a name tag in the peer's colour, in every other peer's editor. Its server relays presence over a second socket, and each peer frame learns its name and colour from its URL.
- "Add a peer" only shows for examples with a server, since peers without one share nothing, and a new peer scrolls into view.
