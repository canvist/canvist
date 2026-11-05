---
"canvist_cloud": security
"canvist_dictation_relay": security
---

# Remove the unmaintained pin-utils dependency from the HTTP stack

Update Hyper from 1.8.1 to 1.12.0 within its existing compatible version range. This removes pin-utils, flagged by informational advisory RUSTSEC-2026-0441, without adding a cargo-deny exception.
