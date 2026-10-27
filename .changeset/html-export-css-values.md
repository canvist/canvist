---
canvist_extensions: fix
---

# Prevent CSS injection in exported HTML

Validate alignment, highlight colors, and text-style values when exporting HTML, including values imported from JSON or collaboration. Unsupported CSS expressions, escapes, comments, and declaration delimiters are omitted while text and valid styles are preserved. Supported styles include simple alignment keywords, hex and named colors, flat color functions, quoted or unquoted font family lists, and nonnegative sizes in supported units.
