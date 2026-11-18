---
"canvist-dart": fix
---

# Keep a view's fonts in step when its first width starts layout over

A view laid out before it had a width starts layout over at its first real width, and the new layout numbers its fonts from zero. The host now learns them again, so text no longer measures and paints in another block's font, such as body text in the heading's bold, after a frame at zero width.
