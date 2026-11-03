---
"@canvist/web": none
"canvist-rust": none
---

# Make SDK source builds repeatable and unblock testkit health checks

Use the same dependency-ordered browser SDK build in CI and local checkouts, pin wasm-pack for publication, and preserve host build-tool metadata on macOS. Release readiness documentation records the registry setup and distribution work still required before the first release. An idle testkit agent now releases the daemon mutex while polling, so health checks and queued work remain responsive instead of starting competing device runs.
