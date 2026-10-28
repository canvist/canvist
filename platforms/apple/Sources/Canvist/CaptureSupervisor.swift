@preconcurrency import AVFAudio
import os

/// Keeps a capture going through the audio session's changes while a
/// dictation session runs.
///
/// - An interruption (a phone call, Siri, another app taking the audio
///   session) ends the session as stopping does: the words already said
///   become final. It does not resume when the interruption ends; the
///   person starts again.
/// - A new input route (headphones with a microphone connecting or
///   disconnecting) or a change to the engine's configuration restarts
///   capture on the new route, and ends the session as stopping does when
///   it cannot.
///
/// The system posts these notifications on its own threads; the supervisor
/// acts on them on the main actor.
final class CaptureSupervisor: NSObject, @unchecked Sendable {
  /// What a notification means for a capture.
  enum Event: Equatable, Sendable {
    /// The audio session was taken away.
    case interrupted
    /// The input route or the engine's format changed.
    case inputChanged
  }

  private let notifications: NotificationCenter
  private let respond: @MainActor @Sendable (Event) -> Void
  /// Whether events still matter: false once the session is over.
  @MainActor private var watching = true

  /// Watch the shared audio session and `input`'s engine, ending the
  /// session with `end` when capture cannot go on.
  @MainActor
  convenience init(
    input: any AudioInput,
    notifications: NotificationCenter,
    end: @escaping @MainActor () -> Void
  ) {
    self.init(notifications: notifications, engine: input.engine) { event in
      Self.handle(event, input: input, end: end)
    }
  }

  private init(
    notifications: NotificationCenter,
    engine: AnyObject,
    respond: @escaping @MainActor @Sendable (Event) -> Void
  ) {
    self.notifications = notifications
    self.respond = respond
    super.init()

    let observe = #selector(observe(_:))
    notifications.addObserver(
      self, selector: observe, name: AVAudioSession.interruptionNotification, object: nil)
    notifications.addObserver(
      self, selector: observe, name: AVAudioSession.routeChangeNotification, object: nil)
    notifications.addObserver(
      self, selector: observe, name: .AVAudioEngineConfigurationChange, object: engine)
  }

  /// Stop watching. Events already on their way to the main actor are
  /// dropped.
  @MainActor
  func cancel() {
    watching = false
    notifications.removeObserver(self)
  }

  @objc private func observe(_ notification: Notification) {
    guard let event = Self.event(notification) else { return }

    Task { @MainActor in
      guard watching else { return }

      respond(event)
    }
  }

  /// Finish on an interruption; restart on an input change, finishing
  /// when capture can't restart.
  @MainActor
  private static func handle(_ event: Event, input: any AudioInput, end: @MainActor () -> Void) {
    if event == .interrupted {
      logger.notice("the audio session was interrupted: finishing dictation")
      end()

      return
    }

    do {
      try input.restart()
    } catch {
      logger.error("restarting capture failed: \(error.localizedDescription, privacy: .public)")
      end()
    }
  }

  private static let logger = Logger(subsystem: "dev.canvist", category: "Dictation")

  /// What `notification` means for a capture, or `nil` when it doesn't
  /// matter: an interruption ending, or a route change that leaves the
  /// input alone.
  static func event(_ notification: Notification) -> Event? {
    switch notification.name {
    case AVAudioSession.interruptionNotification:
      let type = (notification.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt)
        .flatMap(AVAudioSession.InterruptionType.init)

      return type == .began ? .interrupted : nil
    case AVAudioSession.routeChangeNotification:
      let reason = (notification.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt)
        .flatMap(AVAudioSession.RouteChangeReason.init)

      return reason.map(inputRouteReasons.contains) == true ? .inputChanged : nil
    case .AVAudioEngineConfigurationChange:
      return .inputChanged
    default:
      return nil
    }
  }

  /// Route changes that can change the microphone.
  private static let inputRouteReasons: Set<AVAudioSession.RouteChangeReason> = [
    .newDeviceAvailable, .oldDeviceUnavailable, .override, .routeConfigurationChange,
  ]
}
