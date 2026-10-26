---
canvist_scene: security
---

# Bound parent ancestry work when loading scene JSON

Resolve parent depths once with an iterative traversal, preserving stable depth ordering and deterministic cycle errors for flat parent chains.
