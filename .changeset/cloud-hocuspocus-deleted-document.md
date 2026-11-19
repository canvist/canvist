---
"rust:canvist_cloud": fix
---

# Hocuspocus sockets let go of deleted documents

Deleting a document left it open on Hocuspocus sockets, held by a room nothing saved. Authenticating the same name again on that socket, as a provider created again for it does, kept editing the deleted room: edits were acknowledged but never stored or shared. The socket now closes the document when it is deleted, so the next authentication joins a new room.
