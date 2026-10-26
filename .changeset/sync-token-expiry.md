---
"canvist_cloud": patch
---

# Enforce sync token expiry throughout a connection

Close live and paused sync connections when their client tokens expire. Validate every Hocuspocus token refresh and replace its scope and lifetime, closing expired sockets with code 1008.
