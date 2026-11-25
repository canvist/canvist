---
"website": perf
---

# Ship only the font weights the website uses

The subset fonts keep only the weight range the site sets: 400 to 800 for Bricolage Grotesque and 400 to 700 for Geist Mono, which drops the variation data for the lighter and heavier weights nobody sees. Every page downloads 6.6 KiB less font (67.2 KiB to 60.6 KiB), about 8% of a docs page. Text renders the same; screenshots at every weight the site uses differ only in antialiasing.
