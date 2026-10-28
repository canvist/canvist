@preconcurrency import AVFAudio
import Foundation
import Speech
import os

/// Dictation with the device's own recognizer, `SpeechAnalyzer`: for
/// offline use, and for apps without a relay.
///
/// Speech never leaves the device. The trade-off is that there is no polish
/// pass, so fillers and spoken corrections stay in the text. The first
/// session in a language may download its model.
@available(iOS 26, *)
@MainActor
public final class OnDeviceDictationSource: DictationSource {
  private let locale: Locale
  private let input: any AudioInput
  private let notifications: NotificationCenter
  /// Stops the microphone and ends the audio stream, so the analyzer
  /// finalizes what it heard and its results end. Set while a session runs.
  private var endAudio: (() -> Void)?
  /// Whether ``finish()`` was called during this session, perhaps before
  /// the microphone started.
  private var finishing = false
  /// The segment the recognizer is reporting now.
  private var segment: UInt32 = 0
  private let logger = Logger(subsystem: "dev.canvist", category: "Dictation")

  /// Dictation in `locale`'s language, the person's own by default.
  public convenience init(locale: Locale = .current) {
    self.init(locale: locale, input: MicrophoneInput())
  }

  init(locale: Locale, input: any AudioInput, notifications: NotificationCenter = .default) {
    self.locale = locale
    self.input = input
    self.notifications = notifications
  }

  /// Whether this device has an on-device recognizer.
  public nonisolated static var isAvailable: Bool {
    SpeechTranscriber.isAvailable
  }

  public func run(
    into editor: Editor, progress: @escaping @MainActor (DictationProgress) -> Void
  ) async throws {
    segment = 0
    try await Self.authorize()

    let transcriber = try await transcriber()
    let modules: [any SpeechModule] = [transcriber]

    guard let format = await SpeechAnalyzer.bestAvailableAudioFormat(compatibleWith: modules) else {
      throw DictationError.recognizerUnavailable
    }

    let (audio, inputs) = AsyncStream.makeStream(of: AnalyzerInput.self)
    let analyzer = SpeechAnalyzer(modules: modules)
    let converter = PCMConverter(output: format)

    endAudio = {
      self.input.stop()
      inputs.finish()
    }

    defer {
      endAudio?()
      endAudio = nil
      finishing = false
    }

    // Capture before analyzing: a refused microphone starts no analyzer,
    // and the first words wait in the stream.
    try await input.start(
      converter.converting { converted in inputs.yield(AnalyzerInput(buffer: converted)) })

    // Stopping while the microphone was starting stops it now.
    if finishing {
      finish()
    }

    // A phone call or a new route mid-session is handled as stopping is,
    // so the words already said still become final.
    let supervisor = CaptureSupervisor(input: input, notifications: notifications) {
      progress(.finishing)
      self.finish()
    }
    defer { supervisor.cancel() }

    try await analyzer.start(inputSequence: audio)
    progress(.listening)

    do {
      try await withTaskCancellationHandler {
        try await transcribe(transcriber, analyzer, into: editor)
      } onCancel: {
        Task { await analyzer.cancelAndFinishNow() }
      }
    } catch {
      await analyzer.cancelAndFinishNow()

      throw error
    }
  }

  /// Dictate the transcriber's results until the audio ends and the
  /// analyzer has finalized the last words.
  private func transcribe(
    _ transcriber: SpeechTranscriber, _ analyzer: SpeechAnalyzer, into editor: Editor
  ) async throws {
    try await withThrowingTaskGroup(of: Void.self) { group in
      // Waits for the audio to end, then finalizes the last words, which
      // ends the results.
      group.addTask { try await analyzer.finalizeAndFinishThroughEndOfInput() }

      for try await result in transcriber.results {
        try dictate(String(result.text.characters), isFinal: result.isFinal, into: editor)
      }

      try await group.waitForAll()
    }
  }

  public func finish() {
    finishing = true
    endAudio?()
  }

  /// Show or insert what the recognizer said. Its volatile result is the
  /// draft of everything after the last final one, so each final result
  /// closes a segment.
  func dictate(_ text: String, isFinal: Bool, into editor: Editor) throws {
    let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
    _ = try editor.dictate(segment: segment, text: text, isFinal: isFinal)

    if isFinal {
      segment += 1
    }
  }

  /// A transcriber for the locale that reports drafts, with its model
  /// installed.
  private func transcriber() async throws -> SpeechTranscriber {
    guard SpeechTranscriber.isAvailable else {
      throw DictationError.recognizerUnavailable
    }

    guard let supported = await SpeechTranscriber.supportedLocale(equivalentTo: locale) else {
      throw DictationError.unsupportedLanguage(locale.identifier(.bcp47))
    }

    let transcriber = SpeechTranscriber(
      locale: supported,
      transcriptionOptions: [],
      reportingOptions: [.volatileResults],
      attributeOptions: []
    )

    do {
      if let installation = try await AssetInventory.assetInstallationRequest(supporting: [
        transcriber
      ]) {
        try await installation.downloadAndInstall()
      }
    } catch {
      logger.error(
        "installing the speech model failed: \(error.localizedDescription, privacy: .public)")

      throw DictationError.recognizerUnavailable
    }

    return transcriber
  }

  /// Ask for speech recognition, which on-device transcription needs too.
  /// Nonisolated: the system answers on its own queue, and a closure made
  /// on the main actor would assert it runs there.
  private nonisolated static func authorize() async throws {
    let status = await withCheckedContinuation { continuation in
      SFSpeechRecognizer.requestAuthorization { status in continuation.resume(returning: status) }
    }

    guard status == .authorized else {
      throw DictationError.speechRecognitionDenied
    }
  }
}
