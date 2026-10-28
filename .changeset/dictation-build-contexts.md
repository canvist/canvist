---
canvist_cloud: none
website: none
---

# Include the dictation relay in the container build contexts

Cargo reads every workspace member's manifest, even to build one crate, so the canvist Cloud and website images now copy `services/dictation` into their build contexts.
