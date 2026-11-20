All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.2.2](https://github.com/canvist/canvist/releases/tag/v0.2.2) (2026-11-20)

### 🐛 Fixed

- **Bound table spans read from JSON.** A cell's `colspan` and `rowspan` from JSON, which a stored document or a collaborator supplies, sized the table's grid as given. The first structural edit in a table with `"colspan": 4294967296`, such as pressing Enter in a cell, built a grid of billions of slots and padded every other row to that width, and never finished. The table commands, the `tableCell` query, and the table repair after each edit now count spans up to the limits browsers apply, 1,000 columns and 65,534 rows, as pasted HTML already did. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #264](https://github.com/canvist/canvist/pull/264)

<details>
<summary><strong>🔨 Refactor</strong></summary>

#### Write the table spans the grid was built with to HTML

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #267](https://github.com/canvist/canvist/pull/267)

Server-rendered HTML wrote a cell's `colspan` and `rowspan` as the document held them, while the table's grid clamped them. A cell whose JSON claimed a `colspan` of `4294967295` was laid out across 1,000 columns but exported with the full number. The HTML now carries the clamped spans: at most 1,000 columns and 65,534 rows, the limits browsers apply.

`canvist_model` exports those limits as `MAX_COLSPAN` and `MAX_ROWSPAN`, and the table extension and the HTML reader use them in place of their own copies.

</details>
