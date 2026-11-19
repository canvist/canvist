---
"rust:canvist_extension_table": fix
"rust:canvist_headless": fix
"@canvist/headless": fix
---

# Bound table spans read from JSON

A cell's `colspan` and `rowspan` from JSON, which a stored document or a collaborator supplies, sized the table's grid as given. The first structural edit in a table with `"colspan": 4294967296`, such as pressing Enter in a cell, built a grid of billions of slots and padded every other row to that width, and never finished. The table commands, the `tableCell` query, and the table repair after each edit now count spans up to the limits browsers apply, 1,000 columns and 65,534 rows, as pasted HTML already did.
