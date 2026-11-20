All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.2.2](https://github.com/canvist/canvist/releases/tag/v0.2.2) (2026-11-20)

### 🐛 Fixed

#### Escape only what would read as Markdown syntax

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #249](https://github.com/canvist/canvist/pull/249)

Markdown export no longer backslashes every `_`, `*`, `~`, `` ` ``, `[`, `]` and `\` in text. Each is escaped only where it would otherwise read as syntax, judged with the whole line in view: `snake_case`, `a * b`, `[a note]`, `C:\Users` and the `_` and `~` in a bare URL stay as written, while `*stars*`, `[label](href)` and paired backticks are still escaped. Escaped underscores inside bare URLs used to break those links in GitHub-flavoured renderers.

The Markdown reader now treats bare URLs as text, as GitHub does, so a `_` inside one never starts emphasis, and keeps the hard break that two trailing spaces make at the end of a paragraph line.

- **Keep video settings through HTML.** HTML wrote only a video's source and size, so exporting and reading back a document, or copying and pasting a video, lost a YouTube video's `start` and a Twitch video's `autoplay` and `muted`. The frame now carries `start="30"` as TipTap writes it, and `autoplay` and `muted` as boolean attributes, as `<audio>` does, and the reader takes them back. A video that starts from the beginning writes no `start`. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #264](https://github.com/canvist/canvist/pull/264)

<details>
<summary><strong>🔨 Refactor</strong></summary>

#### Write the table spans the grid was built with to HTML

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #267](https://github.com/canvist/canvist/pull/267)

Server-rendered HTML wrote a cell's `colspan` and `rowspan` as the document held them, while the table's grid clamped them. A cell whose JSON claimed a `colspan` of `4294967295` was laid out across 1,000 columns but exported with the full number. The HTML now carries the clamped spans: at most 1,000 columns and 65,534 rows, the limits browsers apply.

`canvist_model` exports those limits as `MAX_COLSPAN` and `MAX_ROWSPAN`, and the table extension and the HTML reader use them in place of their own copies.

</details>
