---
"rust:canvist_ffi": fix
---

# Reading text over a reversed range no longer crashes the app

`Editor.textIn` returns an empty string for a range that ends before it starts, instead of panicking, which Swift turned into a crash and Kotlin into an `InternalException`. Android's input connection also reads text around the caret in 64-bit offsets, so an input method asking for `Int.MAX_VALUE` characters after the caret gets the rest of the text rather than a wrapped, reversed range.
