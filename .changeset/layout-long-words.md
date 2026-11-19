---
"rust:canvist_layout": perf
---

# Lay out long words without rescanning them for every grapheme

A word with no break opportunity, such as a long URL or a pasted token, wraps by being cut between graphemes. Each cut found every grapheme boundary from the cut to the end of the word and measured every grapheme by summing advances from the start of the word, so a 40,000-character word took most of a second to lay out. Cuts now measure only the graphemes they take, and a span inside a word counts the characters before it instead of decoding them.

Relaying out a paragraph that is one 2,000-character word after a keystroke takes 0.12 ms instead of 3.1 ms, one 10,000-character word 1.1 ms instead of 57 ms, and one 40,000-character word 14 ms instead of 0.93 s. Lines break exactly where they did.
