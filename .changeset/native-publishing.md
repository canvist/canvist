---
"@canvist/react-native": none
"@canvist/lynx": none
"canvist_desktop": none
"canvist_dictation": none
"canvist_dictation_relay": none
---

# Publish native integrations alongside every SDK release

Enable React Native, Lynx, desktop, the dictation protocol and the dictation relay in the release configuration. Build and inspect both mobile npm packages before publication. Produce complete Apple distributions for SwiftPM and CocoaPods, and publish Android to Maven Central on real releases. Require all native credentials before any SDK package publication so a missing token cannot leave an apparently successful source-only native release.

Browser CI now accepts runner images that already use the canonical Ubuntu mirror, instead of failing before dependency installation when an Azure mirror search finds no matches.
