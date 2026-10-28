---
"canvist_ffi": minor
---

# Dictate into the Android editor

`canvist-android` can now turn speech into text at the caret, as the iOS package does. The words still being said show faint, rewrite themselves as the speech model revises them, and settle into ink once final. Drafts never enter the document.

- **`CanvistDictation`** dictates into a `CanvistView`, or into an editor without a view, with its `state` as a `StateFlow` (`Idle`, `Connecting`, `Listening`, `Finishing`, `Failed`). Sessions run in the `CoroutineScope` you pass, so they end with your screen. `stop()` lets the last words become final; `cancel()` drops them. A session that fails, ends, or loses its scope leaves no draft behind.
- **`RelayDictationSource`** streams the microphone as 16 kHz PCM16 to canvist's dictation relay over an OkHttp WebSocket, authorized with a token your server mints.
- **Calls and headsets.** A session holds audio focus. Losing it, to a call or another app recording, finishes the session as `stop()` does, so the words already said become final. A microphone connecting or disconnecting moves capture to the new route and keeps the session, or finishes it cleanly when capture can't restart.
- **`OnDeviceDictationSource`** uses Android's `SpeechRecognizer`, preferring the on-device recognizer on Android 12 and newer, and restarts it after each pause until the session stops. It has no polish pass.
- **Permissions.** The library asks for no permissions. Apps add `RECORD_AUDIO` (and `INTERNET` for the relay) and ask for the microphone before starting; a refusal fails the session with `DictationException.MicrophoneDenied`, whose `describe(context)` says what to do.
- **Animated text.** `CanvistView` turns on the editor's animations unless the system's animations are off, and repaints on every display refresh only while a frame is animating, so drafts breathe, final text fades in, and kinetic text moves on Android.

The library now depends on `kotlinx-coroutines-android` and OkHttp 4. The sample app has a mic button that dictates through a relay when built with one, or with the device's recognizer, and the dictation guide (`docs/src/guides/dictation.md`) covers the Android API.
