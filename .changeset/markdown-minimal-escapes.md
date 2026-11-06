---
"canvist_extensions": fix
---

# Escape only what would read as Markdown syntax

Markdown export no longer backslashes every `_`, `*`, `~`, `` ` ``, `[`, `]` and `\` in text. Each is escaped only where it would otherwise read as syntax, judged with the whole line in view: `snake_case`, `a * b`, `[a note]`, `C:\Users` and the `_` and `~` in a bare URL stay as written, while `*stars*`, `[label](href)` and paired backticks are still escaped. Escaped underscores inside bare URLs used to break those links in GitHub-flavoured renderers.

The Markdown reader now treats bare URLs as text, as GitHub does, so a `_` inside one never starts emphasis, and keeps the hard break that two trailing spaces make at the end of a paragraph line.
