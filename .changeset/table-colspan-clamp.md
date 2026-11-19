---
"rust:canvist_model": fix
"rust:canvist_layout": fix
---

# Clamp table colspans to 1,000 columns

`TableMap` sized a table's grid by every cell's `colspan`, however large. A document whose cell claimed a colspan of `9223372036854775807`, as any JSON can, made layout panic with a capacity overflow, and one of a billion columns tried to allocate gigabytes. A colspan over 1,000 is now clamped to 1,000, as browsers clamp the HTML attribute, so table layout and the table commands see the same grid a browser would.
