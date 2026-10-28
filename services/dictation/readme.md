<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/blob/main/docs/internal/dictation.md"><strong>Design</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/actions/workflows/ci.yml"><img src="https://github.com/canvist/canvist/actions/workflows/ci.yml/badge.svg?branch=main" alt="ci status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_dictation_relay

The dictation relay: canvist clients stream speech to it, and it streams text back as drafts that rewrite themselves while you talk and finals that settle into the document. Every client speaks one protocol ([`canvist_dictation`](../../crates/canvist_dictation)); the relay translates it for the speech provider you choose, so API keys stay on your server and switching providers changes no client. It is not published to crates.io; build the `canvist-dictation` binary from this repository or run the container image.

- **Providers:** OpenAI's Realtime transcription API, Deepgram's streaming API, or a local model served by vLLM (Voxtral Realtime, Qwen3-ASR). Each base URL is configurable, so servers that copy these APIs work too.
- **Turn detection:** a small voice activity detector hears the end of each turn and closes segments for providers that need it.
- **Polish:** optionally, a language model tidies each segment before it is final (punctuation, fillers, "Tuesday, no, Wednesday"). Claude, or any Chat Completions-compatible server: OpenAI, Ollama, vLLM, llama.cpp. When it is slow or fails, the raw transcript is used.
- **Order:** finals reach the client in segment order, however the provider and the polish pass interleave them.

The protocol and the reasons behind it are in [`docs/internal/dictation.md`](../../docs/internal/dictation.md).

## Running it

```sh
CANVIST_DICTATION_TOKEN_SECRET=$(openssl rand -hex 32) \
CANVIST_DICTATION_PROVIDER=deepgram \
CANVIST_DICTATION_PROVIDER_KEY=… \
cargo run --release -p canvist_dictation_relay
```

It listens on `PORT` (8080) and serves:

- `GET /v1/dictation`: the dictation WebSocket.
- `GET /healthz`: 200 while serving, 503 once shutdown begins.

Or build the container from the repository root:

```sh
docker build -f services/dictation/Dockerfile -t canvist-dictation .
docker run -p 8080:8080 --env-file dictation.env canvist-dictation
```

### OpenAI

```sh
CANVIST_DICTATION_PROVIDER=openai
CANVIST_DICTATION_PROVIDER_KEY=sk-…
# Optional: another model, or a server that copies the API.
CANVIST_DICTATION_PROVIDER_MODEL=gpt-live-transcribe
CANVIST_DICTATION_PROVIDER_URL=wss://api.openai.com
```

The relay opens a `transcription` session at 24 kHz with server turn detection off, and commits each turn when its own voice activity detection hears a pause.

### Deepgram

```sh
CANVIST_DICTATION_PROVIDER=deepgram
CANVIST_DICTATION_PROVIDER_KEY=…
CANVIST_DICTATION_PROVIDER_MODEL=nova-3
```

Deepgram finds turns itself and rewrites its interim results, so drafts change the most with it.

### A local model with vLLM

Serve a realtime speech model with vLLM (see the model card for the flags it needs), then point the relay at it:

```sh
vllm serve mistralai/Voxtral-Mini-4B-Realtime-2602

CANVIST_DICTATION_PROVIDER=vllm
CANVIST_DICTATION_PROVIDER_URL=ws://localhost:8000
CANVIST_DICTATION_PROVIDER_MODEL=mistralai/Voxtral-Mini-4B-Realtime-2602
# Only when vLLM was started with --api-key.
CANVIST_DICTATION_PROVIDER_KEY=…
```

vLLM's deltas only ever append, so a segment ends when the relay hears a pause, after `CANVIST_DICTATION_SEGMENT_GRACE_MS` for the words still on their way.

### Polish

```sh
# Claude through the Messages API.
CANVIST_DICTATION_POLISH=anthropic
CANVIST_DICTATION_POLISH_KEY=sk-ant-…

# Or any Chat Completions-compatible server, such as a local Ollama.
CANVIST_DICTATION_POLISH=openai
CANVIST_DICTATION_POLISH_URL=http://localhost:11434/v1
CANVIST_DICTATION_POLISH_MODEL=llama3.2
```

Clients can still turn polish off for a session with `"polish": false`. The model is told to fix punctuation and casing, drop fillers and false starts, apply spoken corrections, and never add anything; a reply much longer than the transcript is refused and the raw transcript used instead.

## Configuration

Every setting is an environment variable. Empty values count as unset.

| Variable                                | Default                                                                       | Meaning                                                                        |
| --------------------------------------- | ----------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `CANVIST_DICTATION_TOKEN_SECRET`        | required                                                                      | The HMAC key client tokens are signed with. At least 32 bytes.                 |
| `CANVIST_DICTATION_PROVIDER`            | required                                                                      | `openai`, `deepgram`, or `vllm`.                                               |
| `CANVIST_DICTATION_PROVIDER_KEY`        | required for `openai` and `deepgram`                                          | The provider's API key.                                                        |
| `CANVIST_DICTATION_PROVIDER_URL`        | `wss://api.openai.com`, `wss://api.deepgram.com`, or `ws://localhost:8000`    | The provider's `ws://` or `wss://` origin, without a path.                     |
| `CANVIST_DICTATION_PROVIDER_MODEL`      | `gpt-live-transcribe`, `nova-3`, or `mistralai/Voxtral-Mini-4B-Realtime-2602` | The transcription model.                                                       |
| `CANVIST_DICTATION_SEGMENT_GRACE_MS`    | `600`                                                                         | vLLM only: how long a segment stays open after a pause.                        |
| `CANVIST_DICTATION_POLISH`              | `off`                                                                         | `off`, `anthropic`, or `openai`.                                               |
| `CANVIST_DICTATION_POLISH_KEY`          | required for `anthropic`                                                      | The polish model's API key.                                                    |
| `CANVIST_DICTATION_POLISH_URL`          | `https://api.anthropic.com`, or `https://api.openai.com/v1`                   | The API base. For `openai`, a Chat Completions base ending in `/v1`.           |
| `CANVIST_DICTATION_POLISH_MODEL`        | `claude-haiku-5-5` for `anthropic`; required for `openai`                     | The polish model.                                                              |
| `CANVIST_DICTATION_POLISH_TIMEOUT_MS`   | `3000`                                                                        | The longest one segment's polish may take before the raw transcript is used.   |
| `CANVIST_DICTATION_MAX_SESSION_SECONDS` | `600`                                                                         | The longest a session may run. It then finishes as if the client sent `stop`.  |
| `CANVIST_DICTATION_IDLE_SECONDS`        | `30`                                                                          | How long a session may go without audio, or without its `start`.               |
| `CANVIST_DICTATION_MAX_FRAME_BYTES`     | `65536`                                                                       | The largest message a client may send: two seconds of audio.                   |
| `CANVIST_DICTATION_FINISH_TIMEOUT_MS`   | `10000`                                                                       | How long `stop` waits for the provider before its drafts become finals.        |
| `HOST`, `PORT`                          | `::`, `8080`                                                                  | Where to listen.                                                               |
| `CANVIST_LOG_FORMAT`                    | text                                                                          | `json` for one JSON object per line. `RUST_LOG` filters, defaulting to `info`. |

The relay refuses to start without a key its provider or polish model needs.

## Client tokens

The relay keeps no accounts. Your app's server, which already knows who its user is, mints a short-lived HS256 JWT with the shared secret, and the client presents it as `Authorization: Bearer <token>` or, from a browser, the `token` query parameter:

```json
{
	"sub": "user-123",
	"scope": "dictation",
	"iat": 1767225600,
	"exp": 1767225900
}
```

`scope` must be `dictation`, and `exp` is checked with five seconds of leeway, as canvist Cloud's tokens are. From Rust, `canvist_dictation_relay::auth::mint_token(secret, "user-123", ttl)` signs one; any JWT library works too. A token is checked when the session opens; the session then runs up to its duration limit. A refused token is reported over the socket as an `unauthorized` error, because browsers cannot read a failed handshake's status.

## Behaviour worth knowing

- **Errors.** Protocol mistakes (`bad_request`), refused tokens (`unauthorized`), and oversized frames (`limit`) are fatal and close the socket. The duration and idle limits are not: the session finishes its segments and sends `done`. A failed polish is a non-fatal `polish` error with the raw transcript as the final. A provider that fails is a fatal `provider` error; one that misses the finish timeout is a non-fatal one, and its drafts become finals.
- **Empty finals.** A segment that turned out to hold no words gets an empty `final`, so a draft the client was showing is cleared.
- **Shutdown.** On SIGTERM, `/healthz` turns to 503, new sessions are refused, and open sessions finish as if their clients sent `stop`; their sockets close with 1012 so clients reconnect elsewhere.
- **Privacy.** Audio, transcripts, and keys are never logged. Provider error details go to the log, not to clients, because providers sometimes quote a masked key.

## Performance

Measured on Apple silicon in release mode (`test:perf`): voice activity detection takes 11 µs per second of audio, and resampling to 24 kHz for OpenAI 46 µs, so the relay's own work per session is a rounding error next to the network.

```sh
cargo test --release -p canvist_dictation_relay --test perf_budgets -- --ignored --nocapture
```

The tests run against fake providers and a fake polish model over real sockets; nothing leaves the machine.
