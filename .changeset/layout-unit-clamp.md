---
"rust:canvist_layout": fix
---

# Lay out blocks with absurdly large heights

Layout sums block heights in 1/64 px integers. A height from a document attribute, such as an image's `height`, can be any finite number, and one of `1e30` converted to `i64::MAX` units, so adding the next block's offset overflowed: a panic in debug builds and negative positions in release. Positions and heights now convert to at most 2^31 px, which keeps every sum in range, as browsers saturate their layout units.
