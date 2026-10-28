---
"canvist_ffi": minor
---

# Dictate into the iOS editor

The `Canvist` Swift package can now turn speech into text at the caret. The words still being said show faint, rewrite themselves as the speech model revises them, and settle into ink once final. Drafts never enter the document.

- **`CanvistDictation`** dictates into a `CanvistView`, with an observable `state` (`idle`, `connecting`, `listening`, `finishing`, `failed`) for SwiftUI and UIKit buttons. `stop()` lets the last words become final; `cancel()` drops them. A session that fails or ends leaves no draft behind.
- **`RelayDictationSource`** streams the microphone as 16 kHz PCM16 to canvist's dictation relay over a WebSocket, authorized with a token your server mints.
- **Calls and headsets.** An audio interruption (a call, Siri, another app) finishes the session as `stop()` does, so the words already said become final. A route or engine configuration change moves capture to the new microphone and keeps the session, or finishes it cleanly when capture can't restart.
- **`OnDeviceDictationSource`** uses Apple's `SpeechAnalyzer` on iOS 26 and newer, for offline use and apps without a relay. It has no polish pass.
- **Animated text.** `CanvistView` turns on the editor's animations unless Reduce Motion is on, and repaints on a display link only while a frame is animating, so drafts breathe, final text fades in, and kinetic text moves on iOS.

`canvist_ffi` decodes the relay's messages for the native bindings: `Editor.receiveDictation(message)` applies a relay message and returns a `DictationEvent`, `dictationStartMessage(language, polish)` opens a session with the text before the caret as context, and `dictationStopMessage()` ends one.

The example app has a mic button in its toolbar, and the dictation guide (`docs/src/guides/dictation.md`) covers running the relay, minting tokens, and the Apple API.
