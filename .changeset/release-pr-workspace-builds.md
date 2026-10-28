---
"website": patch
---

# Release pull requests build the site from workspace packages

Release preparation rewrites internal dependencies to ranges (`^0.2.0`) that name versions npm will only have after the release publishes. A relink step now runs before every website and bundle-size deno resolution — including the website image build — repointing those ranges at the workspace packages, so release pull requests pass CI and the site keeps building from local artifacts.
