import Foundation

/// Why dictation failed.
public enum DictationError: Error, Equatable, Sendable, LocalizedError {
  /// The person has not let the app use the microphone.
  case microphoneDenied
  /// The person has not let the app use speech recognition.
  case speechRecognitionDenied
  /// The device's recognizer does not support the language.
  case unsupportedLanguage(String)
  /// The device has no speech recognizer this app can use.
  case recognizerUnavailable
  /// The microphone could not be started, or its audio could not be
  /// converted.
  case audio(String)
  /// The relay ended the session with an error. `message` is for logs.
  case relay(DictationErrorCode, message: String)
  /// The connection to the relay failed or closed before the session
  /// finished.
  case connection(String)

  public var errorDescription: String? {
    switch self {
    case .microphoneDenied:
      String(
        localized: "Allow microphone access in Settings to dictate.",
        comment: "Dictation failed: no microphone permission")
    case .speechRecognitionDenied:
      String(
        localized: "Allow speech recognition in Settings to dictate.",
        comment: "Dictation failed: no speech recognition permission")
    case .unsupportedLanguage:
      String(
        localized: "Dictation doesn't support this language on this device.",
        comment: "Dictation failed: the on-device recognizer lacks the language")
    case .recognizerUnavailable:
      String(
        localized: "Dictation isn't available on this device.",
        comment: "Dictation failed: no on-device recognizer")
    case .audio:
      String(
        localized: "The microphone couldn't start.",
        comment: "Dictation failed: the audio engine failed")
    case .relay(.unauthorized, _):
      String(
        localized: "Dictation isn't authorized. Try again.",
        comment: "Dictation failed: the relay refused the token")
    case .relay(.limit, _):
      String(
        localized: "Dictation reached its limit.",
        comment: "Dictation failed: the relay's session limit")
    case .relay, .connection:
      String(
        localized: "Dictation lost its connection.",
        comment: "Dictation failed: the relay or the network failed")
    }
  }

  /// What went wrong, for logs: never audio, transcripts, or tokens.
  var logDescription: String {
    switch self {
    case .microphoneDenied: "microphone denied"
    case .speechRecognitionDenied: "speech recognition denied"
    case .unsupportedLanguage(let language): "unsupported language \(language)"
    case .recognizerUnavailable: "no on-device recognizer"
    case .audio(let reason): "audio: \(reason)"
    case .relay(let code, let message): "relay \(code): \(message)"
    case .connection(let reason): "connection: \(reason)"
    }
  }
}
