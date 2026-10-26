---
"canvist_cloud": patch
---

# Prevent webhook DNS resolution from reaching private networks

Check DNS answers when configuring webhooks and on every new delivery connection. Reject private or mixed address sets, keep the checked addresses for the connection, and disable proxy and redirect bypasses. Stored webhook URLs are rechecked before delivery. Self-hosters can still explicitly enable private destinations.
