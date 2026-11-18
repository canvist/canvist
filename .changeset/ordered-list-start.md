---
"canvist_layout": fix
---

# Number ordered lists that start near the ends of the integer range

An ordered list's `start` attribute can hold any 64-bit integer. Layout added each item's index to it and the HTML renderer subtracted one from it, so a list starting at `9223372036854775807` panicked in debug builds, and in release numbered its second item `-9223372036854775808.`. Both now stop at the ends of the range.
