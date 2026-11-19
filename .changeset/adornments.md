---
"canvist_core": feat
"canvist_layout": feat
"canvist_web": feat
---

# Adornments: text an extension shows without it being part of the document

`Extension::adornments` returns text to show at points in the document, and `Editor::adornments` collects it after every update. `DocumentLayout::set_adornments` lays it out: each adornment sits between the two carets at its point (upstream before it, downstream after it), is drawn faint in its block's style, and never changes document offsets, so hit testing, line edges, and vertical motion stay in document terms. The browser editor passes them to layout after every change.

`Editor::select_with_affinity` sets the selection with the caret on one side of its point, and the browser editor uses it for clicks, so a click keeps the end of a wrapped line, or the side of an adornment, it landed on. A change that alters neither the document nor the selection, such as storing marks for the next character, now keeps the caret's affinity instead of resetting it.
