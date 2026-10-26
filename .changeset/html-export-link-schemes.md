---
canvist_extensions: fix
canvist_layout: fix
canvist_headless: fix
---

# Reject executable link schemes in HTML exports

Clipboard and headless HTML export omit href for links with unsupported schemes while preserving text and formatting. Both HTML serializers use the same navigation policy and continue escaping allowed attribute values.
