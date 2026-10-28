# Dictation

Dictation turns speech into text in the document as you talk. The words you are still saying appear faint at the caret, rewrite themselves as the speech model changes its mind, and settle into ink once they are final. This page records how it is built and why.

## Decisions

1. **Drafts are local.** Text that may still change is never part of the document. It isn't sent to collaborators, saved, exported, or recorded in history. Only final text is inserted, and each final segment is one undo step.
2. **The editor paints drafts.** `canvist_layout` lays out a block with its provisional text inserted at the caret, so text after it reflows exactly as it will once the text is final. The provisional text is drawn faint, and final text fades into its ink colour on the animation clock that kinetic text already uses. Every platform paints the same display list, so every platform gets the same effect.
3. **The relay is the main source.** Hosted and self-hosted speech models rewrite their drafts, punctuate, and take a language model's cleanup far better than the operating system's recognizers. Every client speaks one canvist protocol to a relay, and the relay translates it to each provider. API keys stay on the server, and adding a provider changes no client.
4. **Platform recognizers are a fallback.** Apple's `SpeechAnalyzer` and Android's `SpeechRecognizer` feed the same editor calls, for offline use and for apps without a relay.

## Layers

```text
microphone ─► host (Swift, Kotlin, TS) ─► relay ─► OpenAI · Deepgram · vLLM (Voxtral, Qwen3-ASR)
                     ▲                       └──► polish: Claude · any OpenAI-compatible chat model
                     │ partial / final
                     ▼
canvist_core: Dictation state (segments) ─► canvist_layout: provisional text + settling
```

- **`canvist_core`** holds the dictation state on `EditorState`, beside the IME composition: an ordered list of segments, each provisional or final. `Editor::dictate` applies a partial or final segment. Final segments commit in order at the selection, as typing does, each in its own transaction and undo step. The state also remembers the ranges committed in the last moments and when, so hosts can animate them settling.
- **`canvist_layout`** takes the provisional text as `DocumentLayout::set_provisional`. The block holding it is laid out with the text inserted, and every query that takes or returns a point in that block maps between document offsets and laid-out offsets. A caret at the insertion point shows after the provisional text.
- **`canvist_dictation`** defines the wire protocol below: serde types shared by the relay and every Rust client binding.
- **`canvist_dictation_relay`** (`services/dictation`) is the relay: an axum WebSocket endpoint, the provider adapters, voice activity detection, and the polish pass. It runs on its own and can be mounted into canvist Cloud.

## Wire protocol

One WebSocket per dictation session at `GET /v1/dictation`. The client authenticates with `Authorization: Bearer <token>`, or the `token` query parameter where a WebSocket can't set headers (browsers).

### Client to relay

The first message is `start`. Audio follows as binary frames, then `stop`.

```json
{
	"type": "start",
	"language": "en",
	"context": "The text before the caret, for punctuation and names.",
	"polish": true
}
```

| Field      | Type    | Meaning                                                                                   |
| ---------- | ------- | ----------------------------------------------------------------------------------------- |
| `language` | string? | A BCP 47 language tag. Omitted means the provider detects it.                             |
| `context`  | string? | Up to 2,000 characters of text before the caret. Used to bias names and punctuation only. |
| `polish`   | bool?   | Run the polish pass when the relay has one configured. Defaults to `true`.                |

- **Audio** is binary frames of 16-bit little-endian PCM, mono, at 16 kHz. Frames may be any length; 20 to 100 ms is typical. The relay resamples for providers that want another rate.
- **`{ "type": "stop" }`** says no more audio is coming. The relay finishes every open segment, sends `done`, and closes.

### Relay to client

```json
{ "type": "ready" }
{ "type": "partial", "segment": 0, "text": "the meeting is on tuesday" }
{ "type": "partial", "segment": 0, "text": "the meeting is on Tuesday no Wednesday" }
{ "type": "final", "segment": 0, "text": "The meeting is on Wednesday." }
{ "type": "error", "code": "provider", "message": "…", "fatal": false }
{ "type": "done" }
```

- **Segments** are numbered from 0 in each session and only increase. A `partial` replaces everything the client shows for its segment. A `final` replaces it for the last time; the segment never changes again.
- **Finals arrive in segment order.** The relay holds a later segment's final until every earlier one is sent.
- **Segment text stands alone**, with no leading or trailing space. Clients join segments, and join the first to the text before the caret, with one space unless the text starts with punctuation or the caret follows white space.
- **Errors** carry a `code`: `unauthorized`, `bad_request`, `provider`, `polish`, `limit`, or `internal`. A fatal error is followed by the socket closing; a non-fatal one (such as a failed polish, which falls back to the raw transcript) is not.

### Polish

With polish on, each segment's transcript goes through a language model before it is final. The model fixes punctuation and casing, drops fillers and false starts, and applies spoken self-corrections ("Tuesday, no, Wednesday" becomes "Wednesday"). It must not add content. While the model works, the relay sends the raw transcript as a `partial`, so the client keeps showing it as a draft. If the model fails or takes longer than its timeout, the raw transcript becomes the `final`, with a non-fatal `polish` error.

## Providers

| Provider   | Use it for                                                                   | How the relay drives it                                                                                                                                       |
| ---------- | ---------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `openai`   | OpenAI, and self-hosted servers that speak its GA Realtime transcription API | `wss://…/v1/realtime?intent=transcription`, a `transcription` session at 24 kHz. The relay commits each turn when its voice activity detection hears a pause. |
| `deepgram` | Hosted streaming with true rewriting of interim results                      | `wss://api.deepgram.com/v1/listen` with `interim_results`. Each `is_final` result closes a segment.                                                           |
| `vllm`     | Local open models: Voxtral Realtime, Qwen3-ASR                               | vLLM's `/v1/realtime` dialect at 16 kHz. Deltas only append, so a pause in speech closes a segment.                                                           |

Polish runs through `anthropic` (the Messages API) or `openai` (any Chat Completions-compatible server: OpenAI, Ollama, vLLM, llama.cpp).

## Painting

- **Provisional text** is drawn in the run's colour at 45% opacity. While the clock runs, it breathes gently between 35% and 55%, so a draft reads as alive.
- **Settling text** fades from 45% to full opacity over 450 ms with an ease-out curve, from the moment its segment is committed.
- With no animation clock (reduced motion), provisional text is a steady 45% and final text appears at full opacity at once.

## Not yet

- **Voice commands** ("make that a heading"). The commands and their tool definitions exist (`engine.metadata()`), so a later polish mode can return a tool call instead of text.
- **canvist Cloud.** Mounting the relay in Cloud needs a metered unit for audio minutes; the business model decides it.
- **Text to speech.** Reading aloud with each word highlighted as it is spoken is a separate, smaller feature.
