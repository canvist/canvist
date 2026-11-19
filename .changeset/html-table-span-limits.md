---
"rust:canvist_layout": fix
"rust:canvist_model": change
"rust:canvist_extension_table": refactor
"rust:canvist_extension_clipboard": refactor
"@canvist/ssr": fix
---

# Write the table spans the grid was built with to HTML

Server-rendered HTML wrote a cell's `colspan` and `rowspan` as the document held them, while the table's grid clamped them. A cell whose JSON claimed a `colspan` of `4294967295` was laid out across 1,000 columns but exported with the full number. The HTML now carries the clamped spans: at most 1,000 columns and 65,534 rows, the limits browsers apply.

`canvist_model` exports those limits as `MAX_COLSPAN` and `MAX_ROWSPAN`, and the table extension and the HTML reader use them in place of their own copies.
