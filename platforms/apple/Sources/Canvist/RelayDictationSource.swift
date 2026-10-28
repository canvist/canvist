@preconcurrency import AVFAudio
import Foundation
import os

/// Dictation through canvist's relay (`services/dictation`): the microphone
/// streams to a speech model on your server, and drafts and final text
/// stream back.
///
/// The relay is the source to reach for first. Hosted and self-hosted
/// models revise their drafts, punctuate, and take an optional language
/// model's polish far better than the device's recognizer.
///
/// `token` is a short-lived token your app's server mints for the person
/// signed in; never ship the relay's secret or a provider's key in the app.
@MainActor
public final class RelayDictationSource: DictationSource {
  private let language: String?
  private let polish: Bool
  private let input: any AudioInput
  private let notifications: NotificationCenter
  private let connect: @MainActor () -> any DictationTransport
  /// Stops the microphone and ends the audio stream, so the stop message
  /// follows the last audio sent. Set while a session runs.
  private var endAudio: (() -> Void)?
  /// Whether ``finish()`` was called during this session, perhaps before
  /// the microphone started.
  private var finishing = false
  private let logger = Logger(subsystem: "dev.canvist", category: "Dictation")

  /// Dictation through the relay at `url`, such as
  /// `wss://dictation.example.com/v1/dictation`, authorized by `token`.
  /// `language` is a BCP 47 tag, or `nil` for the speech model to detect
  /// it; `polish` asks for the relay's language-model cleanup when it has
  /// one.
  public convenience init(url: URL, token: String, language: String? = nil, polish: Bool = true) {
    self.init(language: language, polish: polish, input: MicrophoneInput()) {
      WebSocketTransport(url: url, token: token)
    }
  }

  init(
    language: String?,
    polish: Bool,
    input: any AudioInput,
    notifications: NotificationCenter = .default,
    connect: @escaping @MainActor () -> any DictationTransport
  ) {
    self.language = language
    self.polish = polish
    self.input = input
    self.notifications = notifications
    self.connect = connect
  }

  public func run(
    into editor: Editor, progress: @escaping @MainActor (DictationProgress) -> Void
  ) async throws {
    let (audio, chunks) = AsyncStream.makeStream(of: Data.self)

    endAudio = {
      self.input.stop()
      chunks.finish()
    }

    defer {
      endAudio?()
      endAudio = nil
      finishing = false
    }

    let converter = try PCMConverter.relay()

    // Capture before connecting: a refused microphone opens no socket, and
    // the first words wait in the stream while the relay connects.
    try await input.start(
      converter.converting { converted in chunks.yield(PCMConverter.littleEndianBytes(converted)) })

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

    let transport = connect()
    defer { transport.close() }

    try await transport.send(.string(editor.dictationStartMessage(language: language, polish: polish)))
    try await withThrowingTaskGroup(of: Bool.self) { group in
      group.addTask {
        try await Self.send(audio, over: transport)

        return false
      }
      group.addTask {
        try await self.receive(from: transport, into: editor, progress: progress)

        return true
      }

      // The session is over once the relay says so (`true`), whether or not
      // the audio was all sent: the relay may end it at its time limit.
      while try await group.next() == false {}

      group.cancelAll()
    }
  }

  public func finish() {
    finishing = true
    endAudio?()
  }

  /// Send every chunk of audio, then the stop message once the audio ends.
  private nonisolated static func send(
    _ audio: AsyncStream<Data>, over transport: any DictationTransport
  ) async throws {
    for await chunk in audio {
      try await transport.send(.data(chunk))
    }

    try Task.checkCancellation()
    try await transport.send(.string(dictationStopMessage()))
  }

  /// Apply the relay's messages to `editor` until it says the session is
  /// done.
  private func receive(
    from transport: any DictationTransport,
    into editor: Editor,
    progress: @MainActor (DictationProgress) -> Void
  ) async throws {
    while true {
      guard case .string(let message) = try await transport.receive() else {
        logger.notice("ignored a binary message from the relay")

        continue
      }

      switch try editor.receiveDictation(message: message) {
      case .ready:
        progress(.listening)
      case .transcript:
        continue
      case .error(let code, let message, true):
        throw DictationError.relay(code, message: message)
      case .error(let code, _, false):
        // Such as a failed polish: the raw transcript becomes final.
        logger.notice("the relay reported \(String(describing: code), privacy: .public)")
      case .done:
        return
      }
    }
  }
}

/// A WebSocket to the relay.
protocol DictationTransport: Sendable {
  /// Send a text or binary message.
  func send(_ message: URLSessionWebSocketTask.Message) async throws

  /// The next message. Throws once the socket closes.
  func receive() async throws -> URLSessionWebSocketTask.Message

  /// Close the socket. Safe to call more than once.
  func close()
}

/// The relay's WebSocket over `URLSession`, authorized with a bearer token.
final class WebSocketTransport: DictationTransport {
  private let task: URLSessionWebSocketTask

  init(url: URL, token: String) {
    var request = URLRequest(url: url)
    request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
    task = URLSession.shared.webSocketTask(with: request)
    task.maximumMessageSize = Self.maximumMessageSize
    task.resume()
  }

  func send(_ message: URLSessionWebSocketTask.Message) async throws {
    do {
      try await task.send(message)
    } catch {
      throw DictationError.connection(error.localizedDescription)
    }
  }

  func receive() async throws -> URLSessionWebSocketTask.Message {
    // A waiting receive ignores task cancellation; closing the socket
    // ends it.
    let task = task

    do {
      return try await withTaskCancellationHandler {
        try await task.receive()
      } onCancel: {
        task.cancel(with: .goingAway, reason: nil)
      }
    } catch {
      try Task.checkCancellation()

      throw DictationError.connection(error.localizedDescription)
    }
  }

  func close() {
    task.cancel(with: .normalClosure, reason: nil)
  }

  /// The relay sends small JSON messages; a draft is a few hundred bytes.
  private static let maximumMessageSize = 1 << 20
}
