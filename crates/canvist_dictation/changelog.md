All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.2.0](https://github.com/canvist/canvist/releases/tag/v0.2.0) (2026-10-30)

### 🚀 Feature

#### Add the dictation protocol and a self-hostable relay

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #230](https://github.com/canvist/canvist/pull/230)

Dictation streams speech to a relay and gets text back as drafts that rewrite themselves and finals that never change again (`docs/internal/dictation.md`).

- `canvist_dictation`: the wire protocol every client and the relay share. `ClientMessage` (`start`, `stop`), `RelayMessage` (`ready`, `partial`, `final`, `error`, `done`), `ErrorCode`, JSON encoding, and the audio format: 16 kHz mono PCM16. It depends only on serde and serde_json.
- `canvist_dictation_relay`: the `canvist-dictation` server. It serves `GET /v1/dictation` to clients holding an HS256 token with the `dictation` scope, and drives OpenAI's Realtime transcription API, Deepgram's streaming API, or a local model on vLLM, each at a configurable base URL. An energy voice activity detector ends turns for providers that need it. An optional polish pass through Claude or any Chat Completions-compatible model tidies each segment, falling back to the raw transcript when it fails or takes longer than its timeout. Finals always reach the client in segment order. Sessions have duration, idle, and frame-size limits, and shut down gracefully.

### 📝 Changed

- **Encode and decode client-side protocol messages by hand.** `RelayMessage::from_json` now reads a `serde_json::Value` instead of going through the derived decoder for internally tagged enums, and `ClientMessage::to_json` writes its text directly. Native clients that decode relay messages carry 33 KiB less code; tests hold both to the derived forms, byte for byte. `ClientMessage::to_json` returns a `String`, since it can no longer fail. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #231](https://github.com/canvist/canvist/pull/231)
