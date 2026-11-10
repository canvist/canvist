---
"canvist_extensions-rust": fix
"canvist_headless": fix
"@canvist/headless": fix
"@canvist/web": fix
"canvist-dart": fix
---

# Fix bugs found reviewing every extension

- `linkHref` returned the first link of a selection spanning two different links; it is now null, like the other style queries. Typed URLs after `(`, `[`, `<`, or a quote now autolink, as pasted ones already did.
- Enter with text selected in a code block replaced the selection with a line break instead of splitting the block.
- Character and word counts no longer split a word or character where its marks change.
- `replace` treats a match selected backwards as the current match.
- `InkHandle::insert` clamps the height to 1 to 4000, as `insertInk` does.
- Comment thread text no longer includes a placeholder character for mentions.
- A suggestion no longer opens for one keystroke when its trigger sits 64 bytes back without white space before it.
- Enter in a details summary whose content starts with an image selects the image.
- Backspace after a mention keeps the mention's marks on the trigger character it leaves.
- `insertHashtag` with an empty tag does nothing.
- `TableHandle::insert` with zero rows or columns does nothing, as `insertTable` does.
- Pasted HTML: column and row spans are capped at 1000 and 65534, `>` inside a quoted attribute no longer ends the tag, and an uppercase `</SCRIPT>` no longer drops the rest of the paste.
- Markdown: link destinations escape `(`, `)`, `<`, `>` and `\`, and read back whole; image alt text no longer gains a backslash on each round trip; code fences are longer than any fence inside the code; links with unsafe schemes are written as plain text, as in HTML; and an ordered list starting at the largest number no longer overflows.
