---
"canvist_scene": fix
---

# Avoid redundant allocations when importing scene JSON

Scene imports reserve their flattened node list up front and share diagnostic prefixes and format node paths only when an error occurs. Large flat parent chains and cycles keep the same validation and deterministic cycle errors while allocating fewer strings and growing fewer vectors.
