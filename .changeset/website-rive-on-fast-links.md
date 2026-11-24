---
"website": perf
---

# Load the homepage's Rive animation only where it is cheap

The hero's Wet Caret animation loads Rive (about 300 KB with Brotli) only on a 4G-or-faster connection, without Save-Data, on a device with at least 4 GB of memory. Everywhere else the CSS stroke paints the wordmark at once, and the homepage costs 92 KB instead of 387 KB. Browsers that report nothing about the connection (Safari, Firefox) still get Rive.
