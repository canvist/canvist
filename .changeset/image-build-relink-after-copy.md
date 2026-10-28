---
"website": patch
---

# The image build relinks after copying the site in

Follow-up to the release pull request workspace builds: the image's deno stage installed from a relinked manifest and then copied the checked-in website over it, restoring dependency ranges that npm cannot resolve yet on a release commit. The relink now runs again after the copy.
