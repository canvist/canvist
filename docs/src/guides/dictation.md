# Dictation

In this guide you let people talk into the editor: run the dictation relay, give your app tokens for it, and start dictating from iOS and Android. The browser is coming.

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

- **Calls and headsets.** A phone call, Siri, or another app taking the microphone finishes the session as `stop()` does: the words already said still land, and the person taps again to carry on. Connecting or disconnecting a headset moves capture to the new microphone without ending the session, or finishes it cleanly when there's no microphone left.

Whatever happens (a fatal relay error, a dropped connection, or a cancel), the draft disappears with the session.

## Dictate on the device

On iOS 26 and newer, `OnDeviceDictationSource` uses Apple's `SpeechAnalyzer` instead of a relay. Speech never leaves the phone, and it works offline:

```swift
if #available(iOS 26, *), OnDeviceDictationSource.isAvailable {
    dictation.start(source: OnDeviceDictationSource(locale: .current))
}
```

There's no polish pass on the device, so fillers and spoken corrections ("Tuesday, no, Wednesday") stay in the text. The first session in a language may download its model. Prefer the relay when you have one, and fall back to the device when you don't.

## Dictate on Android

On Android, `CanvistDictation` works the same way. It takes a `CoroutineScope` too: sessions run in it, so pass your screen's scope and a session never outlives the screen.

```kotlin
import io.github.ifiokjr.canvist.CanvistDictation
import io.github.ifiokjr.canvist.CanvistView
import io.github.ifiokjr.canvist.RelayDictationSource

/**
 * `relay` is such as `wss://dictation.example.com/v1/dictation`, and
 * `token` comes from your server.
 */
@Composable
fun NoteScreen(view: CanvistView, relay: String, token: String) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val dictation = remember { CanvistDictation(view, scope) }
    val state by dictation.state.collectAsState()

    Column {
        TextButton(
            onClick = {
                if (state.isActive) {
                    dictation.stop()
                } else {
                    dictation.start(RelayDictationSource(context, relay, token))
                }
            },
        ) {
            Text(if (state.isActive) "Stop" else "Dictate")
        }
        AndroidView(factory = { view })
    }
}
```

- **`state`** is a `StateFlow` of `Idle`, `Connecting`, `Listening`, `Finishing`, or `Failed(error)`. Outside Compose, collect it in your lifecycle's scope. `error` is a `DictationException`; `error.describe(context)` gives a sentence you can show.
- **`RelayDictationSource(context, url, token, language, polish)`** captures the microphone at 16 kHz and streams it to the relay. `language` and `polish` mean what they do on iOS.
- **`cancel()`** ends a session at once and drops the words that aren't final. So does the scope ending.
- **Permissions.** The library asks for none of its own. Add `RECORD_AUDIO` to your manifest, and `INTERNET` for the relay, then ask for `RECORD_AUDIO` before starting a session, for example with `ActivityResultContracts.RequestPermission`. Without it, the session fails with `DictationException.MicrophoneDenied`.

```xml
<uses-permission android:name="android.permission.RECORD_AUDIO" />
<uses-permission android:name="android.permission.INTERNET" />
```

- **Calls and headsets.** A session holds audio focus while it listens, so music pauses. A phone call or another app recording takes focus away, and that finishes the session as `stop()` does: the words already said still land. Connecting or disconnecting a microphone, such as a headset's, moves capture to it without ending the session, or finishes it cleanly when capture can't restart.

### On the device

`OnDeviceDictationSource` uses Android's `SpeechRecognizer` instead of a relay. On Android 12 and newer it uses the recognizer on the device where there is one, so speech stays on the phone; otherwise it uses the system's recognition service, which may send audio to its provider.

```kotlin
if (OnDeviceDictationSource.isAvailable(context)) {
    dictation.start(OnDeviceDictationSource(context))
}
```

`SpeechRecognizer` stops listening after each pause. The source starts it again until the session stops, and each pause closes a segment. As on iOS, there's no polish pass, so prefer the relay when you have one.

## Reduced motion

Drafts breathe gently and final text fades into ink. When someone turns on **Reduce Motion** on iOS, or **Remove animations** on Android, `CanvistView` turns animations off: drafts show at a steady faint opacity, and final text appears in full ink at once. The view follows the setting as it changes, and only asks for frames while text is animating.
