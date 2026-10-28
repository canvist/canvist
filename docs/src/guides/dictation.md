# Dictation

In this guide you let people talk into the editor: run the dictation relay, give your app tokens for it, and start dictating from iOS. Android and the browser are coming.

## What people see

The words someone is still saying appear faint at the caret. As the speech model changes its mind, they rewrite themselves in place: "the meeting is on tuesday" becomes "The meeting is on Wednesday." Once a phrase is final it settles into ink and the faint text moves on.

Only final text is part of the document. Drafts are never sent to collaborators, saved, exported, or added to history, and each final phrase is one undo step.

## Run the relay

Speech goes to canvist's dictation relay, a small server you run next to your app. It streams audio to the speech model you pick (OpenAI, Deepgram, or a local model on vLLM), can tidy each phrase with a language model, and streams text back. Your provider keys stay on the server, and switching providers changes no app.

```sh
CANVIST_DICTATION_TOKEN_SECRET=$(openssl rand -hex 32) \
CANVIST_DICTATION_PROVIDER=deepgram \
CANVIST_DICTATION_PROVIDER_KEY=… \
cargo run --release -p canvist_dictation_relay
```

The relay listens on port 8080 and serves the dictation WebSocket at `/v1/dictation`. Put it behind TLS so apps connect with `wss://`. The [relay's readme](https://github.com/canvist/canvist/blob/main/services/dictation/readme.md) covers every provider, the polish pass, the container image, and its settings.

## Give your app tokens

The relay keeps no accounts. Your app's server, which already knows who is signed in, mints a short-lived token for each session: an HS256 JWT signed with `CANVIST_DICTATION_TOKEN_SECRET`, with `scope` set to `dictation`.

```json
{
	"sub": "user-123",
	"scope": "dictation",
	"iat": 1767225600,
	"exp": 1767225900
}
```

Any JWT library signs one. From Rust, `canvist_dictation_relay::auth::mint_token(secret, "user-123", ttl)` does it for you. Your app asks your server for a token when someone starts dictating.

Never ship the token secret or a provider's key in an app. Anyone can read them out of the binary.

## Dictate on iOS

`CanvistDictation` dictates into a `CanvistView`. Start it with a source, and stop it when the person is done; words already said still become final:

```swift
import Canvist

@MainActor
@Observable
final class NoteModel {
    let view: CanvistView
    let dictation: CanvistDictation
    /// Such as `wss://dictation.example.com/v1/dictation`.
    let relay: URL

    init(relay: URL) throws {
        view = try CanvistView()
        dictation = CanvistDictation(view: view)
        self.relay = relay
    }

    /// `token` comes from your server.
    func toggleDictation(token: String) {
        if dictation.state.isActive {
            dictation.stop()
            return
        }

        dictation.start(source: RelayDictationSource(url: relay, token: token))
    }
}
```

- **`state`** is `idle`, `connecting`, `listening`, `finishing`, or `failed(DictationError)`. It's observable, so a SwiftUI button reads it directly. `DictationError` has a message you can show.
- **`RelayDictationSource(url:token:language:polish:)`** captures the microphone and streams it to the relay. `language` is a BCP 47 tag, or `nil` for the model to detect it. `polish` asks for the relay's cleanup when it has one.
- **`cancel()`** ends a session at once and drops the words that aren't final.
- **Permissions.** Add `NSMicrophoneUsageDescription` to your Info.plist, and `NSSpeechRecognitionUsageDescription` for on-device dictation. iOS asks the first time someone dictates. A refusal fails the session with `microphoneDenied` or `speechRecognitionDenied`.

Whatever happens (a fatal relay error, a dropped connection, or a cancel), the draft disappears with the session.

## Dictate on the device

On iOS 26 and newer, `OnDeviceDictationSource` uses Apple's `SpeechAnalyzer` instead of a relay. Speech never leaves the phone, and it works offline:

```swift
if #available(iOS 26, *), OnDeviceDictationSource.isAvailable {
    dictation.start(source: OnDeviceDictationSource(locale: .current))
}
```

There's no polish pass on the device, so fillers and spoken corrections ("Tuesday, no, Wednesday") stay in the text. The first session in a language may download its model. Prefer the relay when you have one, and fall back to the device when you don't.

## Reduced motion

Drafts breathe gently and final text fades into ink. When someone turns on **Reduce Motion**, `CanvistView` turns animations off: drafts show at a steady faint opacity, and final text appears in full ink at once. The view follows the setting as it changes, and only asks for frames while text is animating.
