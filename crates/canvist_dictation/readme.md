<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://docs.rs/canvist_dictation"><strong>API</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/actions/workflows/ci.yml"><img src="https://github.com/canvist/canvist/actions/workflows/ci.yml/badge.svg?branch=main" alt="ci status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_dictation

The wire protocol for dictation in [canvist](https://github.com/canvist/canvist): the messages a client and a dictation relay exchange over one WebSocket. The relay ([`canvist_dictation_relay`](../../services/dictation)) and every Rust client binding share these types, so they cannot drift apart.

- **Client to relay:** `start` (language, context, polish), audio as binary frames of 16-bit little-endian PCM, mono, at 16 kHz, then `stop`.
- **Relay to client:** `ready`, `partial` drafts that replace a segment's text, one `final` per segment in segment order, `error` with a code and whether it is fatal, and `done`.

It depends only on serde and serde_json.

```rust
use canvist_dictation::ClientMessage;
use canvist_dictation::RelayMessage;

let start = ClientMessage::Start {
	language: Some("en".to_owned()),
	context: None,
	polish: true,
};
let json = start.to_json();

match RelayMessage::from_json(r#"{"type":"partial","segment":0,"text":"hello"}"#)? {
	RelayMessage::Partial { segment, text } => println!("draft {segment}: {text}"),
	_ => {}
}
# Ok::<(), serde_json::Error>(())
```

The protocol and the design behind it are in [`docs/internal/dictation.md`](../../docs/internal/dictation.md).
