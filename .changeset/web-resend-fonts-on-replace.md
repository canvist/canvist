---
"@canvist/web": fix
"canvist_web": fix
---

# Measure a replaced document in its own fonts

Replacing the document, with `setContent` or when a shared document arrives, starts a new layout that numbers its fonts from zero, but the browser kept the previous document's list. When the new document met its fonts in a different order, text was measured and painted in another block's font, such as body text in the heading's bold. The editor now sends the fonts again after replacing the document.
