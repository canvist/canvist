---
canvist_dictation: change
---

# Encode and decode client-side protocol messages by hand

`RelayMessage::from_json` now reads a `serde_json::Value` instead of going through the derived decoder for internally tagged enums, and `ClientMessage::to_json` writes its text directly. Native clients that decode relay messages carry 33 KiB less code; tests hold both to the derived forms, byte for byte. `ClientMessage::to_json` returns a `String`, since it can no longer fail.
