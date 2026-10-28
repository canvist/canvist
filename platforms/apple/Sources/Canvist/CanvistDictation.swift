import Foundation
import Observation
import os

/// Dictation into a ``CanvistView``: speech becomes text at the caret as
/// someone talks.
///
/// The words still being said show faint at the caret and rewrite
/// themselves as the speech model changes its mind; final words settle into
/// the document, each segment as one undo step. Drafts never enter the
/// document, so collaborators, saves, and undo only ever see final text.
///
/// ```swift
/// let dictation = CanvistDictation(view: view)
/// dictation.start(source: RelayDictationSource(url: relay, token: token))
/// // Later, when the person taps the button again:
/// dictation.stop()
/// ```
///
/// ``state`` is observable, so a SwiftUI button reads it directly, and a
/// UIKit one through `withObservationTracking` (or UIKit's own observation
/// tracking on iOS 18 and later).
@MainActor
@Observable
public final class CanvistDictation {
  /// Where a dictation session is.
  public enum State: Equatable, Sendable {
    /// Not dictating.
    case idle
    /// Asking for permission and reaching the speech model.
    case connecting
    /// Speech is being transcribed.
    case listening
    /// The microphone is off, and the last words are becoming final.
    case finishing
    /// The last session failed. Starting again clears it.
    case failed(DictationError)

    /// Whether a session is running.
    public var isActive: Bool {
      switch self {
      case .connecting, .listening, .finishing: true
      case .idle, .failed: false
      }
    }
  }

  /// Where the current or last session is.
  public private(set) var state = State.idle

  /// The view dictated into.
  public let view: CanvistView

  @ObservationIgnored private var source: (any DictationSource)?
  /// The running session, for tests to await.
  @ObservationIgnored private(set) var session: Task<Void, Never>?
  @ObservationIgnored private let logger = Logger(subsystem: "dev.canvist", category: "Dictation")

  /// Dictation into `view`.
  public init(view: CanvistView) {
    self.view = view
  }

  /// Start dictating at the caret from `source`. Does nothing while a
  /// session is running.
  public func start(source: any DictationSource) {
    guard !state.isActive else { return }

    let editor = view.editor
    self.source = source
    state = .connecting
    // The task holds the controller until the session ends, so it always
    // gets to drop the draft.
    session = Task { await run(source, into: editor) }
  }

  /// Stop listening. Words already said still become final; the session
  /// then ends.
  public func stop() {
    guard state == .connecting || state == .listening else { return }

    state = .finishing
    source?.finish()
  }

  /// End the session now, dropping the words that are not final yet.
  public func cancel() {
    session?.cancel()
  }

  private func run(_ source: any DictationSource, into editor: Editor) async {
    do {
      try await source.run(into: editor) { progress in self.advance(progress) }
      end(.idle)
    } catch {
      end(Self.state(after: error))
    }
  }

  /// How a session that threw `error` ends: cancelling is not a failure.
  private static func state(after error: any Error) -> State {
    if Task.isCancelled {
      return .idle
    }

    if let error = error as? DictationError {
      return .failed(error)
    }

    return .failed(.connection(error.localizedDescription))
  }

  /// Follow the source: listening once speech is being transcribed, and
  /// finishing when it stops listening on its own, such as for a phone
  /// call.
  private func advance(_ progress: DictationProgress) {
    switch (progress, state) {
    case (.listening, .connecting):
      state = .listening
    case (.finishing, .connecting), (.finishing, .listening):
      state = .finishing
    default:
      break
    }
  }

  /// The session is over: whatever was still a draft goes, so nothing
  /// provisional outlives it.
  private func end(_ state: State) {
    _ = view.editor.stopDictation()

    if case .failed(let error) = state {
      logger.error("dictation failed: \(error.logDescription, privacy: .public)")
    }

    source = nil
    session = nil
    self.state = state
  }
}

/// Where dictated speech comes from: canvist's relay
/// (``RelayDictationSource``), or the device's own recognizer
/// (``OnDeviceDictationSource``).
@MainActor
public protocol DictationSource: AnyObject {
  /// Listen, and dictate what is said into `editor` until ``finish()`` or
  /// cancellation. Reports `progress` as the session goes on, returns when
  /// it is over, and throws a ``DictationError`` when it fails.
  /// ``CanvistDictation`` drops any draft left once it returns.
  func run(into editor: Editor, progress: @escaping @MainActor (DictationProgress) -> Void)
    async throws

  /// Stop listening, let the words already said become final, then return
  /// from ``run(into:progress:)``.
  func finish()
}

/// How a dictation session is going, as its source reports it.
public enum DictationProgress: Equatable, Sendable {
  /// Speech is being transcribed.
  case listening
  /// The source stopped listening on its own, as when a phone call takes
  /// the microphone: the words already said are becoming final.
  case finishing
}
