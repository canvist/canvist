@preconcurrency import AVFAudio
import XCTest

@testable import Canvist

/// Dictation through the audio session's changes: calls and Siri taking
/// the microphone, headsets connecting, and the engine reconfiguring. The
/// tests post the system's notifications to their own center.
@MainActor
final class AudioSessionDictationTests: XCTestCase {
  private let notifications = NotificationCenter()
  private let microphone = FakeMicrophone()
  private var view: CanvistView!
  private var dictation: CanvistDictation!

  /// A relay that drafts "world" once started, and makes it final when
  /// stopped.
  private let relay = FakeRelay { message in
    switch message {
    case "start": [#"{"type":"ready"}"#, #"{"type":"partial","segment":0,"text":"world"}"#]
    case "stop": [#"{"type":"final","segment":0,"text":"world."}"#, #"{"type":"done"}"#]
    default: []
    }
  }

  override func setUp() async throws {
    view = try dictationView()
    dictation = CanvistDictation(view: view)
  }

  /// Start a session and wait for its draft.
  private func startDrafting() async throws {
    let source = RelayDictationSource(
      language: nil, polish: true, input: microphone, notifications: notifications
    ) { [relay] in relay }

    dictation.start(source: source)
    try await eventually("the draft") { view.editor.dictationDraft() == " world" }
  }

  private func post(_ name: Notification.Name, object: AnyObject? = nil, _ userInfo: [String: UInt] = [:]) {
    notifications.post(name: name, object: object, userInfo: userInfo)
  }

  private func interruption(_ type: AVAudioSession.InterruptionType) {
    post(AVAudioSession.interruptionNotification, [AVAudioSessionInterruptionTypeKey: type.rawValue])
  }

  private func routeChange(_ reason: AVAudioSession.RouteChangeReason) {
    post(AVAudioSession.routeChangeNotification, [AVAudioSessionRouteChangeReasonKey: reason.rawValue])
  }

  /// The session ended with the draft made final, and nothing provisional
  /// left.
  private func assertFinishedWithFinalText(file: StaticString = #filePath, line: UInt = #line) async {
    await dictation.session?.value

    XCTAssertEqual(dictation.state, .idle, file: file, line: line)
    XCTAssertEqual(view.editor.plainText(), "Hello world.", file: file, line: line)
    XCTAssertNil(view.editor.dictationDraft(), file: file, line: line)
    XCTAssertTrue(microphone.stopped, file: file, line: line)
    XCTAssertEqual(relay.sent.last?.text, #"{"type":"stop"}"#, file: file, line: line)
  }

  func testAnInterruptionMidDraftFinishesWithTheFinalText() async throws {
    try await startDrafting()

    interruption(.began)
    try await eventually("finishing") { dictation.state != .listening }

    await assertFinishedWithFinalText()
  }

  func testAnInterruptionEndingNeitherStopsNorResumes() async throws {
    try await startDrafting()

    interruption(.ended)
    try await Task.sleep(for: .milliseconds(50))
    XCTAssertEqual(dictation.state, .listening)

    interruption(.began)
    await assertFinishedWithFinalText()

    interruption(.ended)
    try await Task.sleep(for: .milliseconds(50))
    XCTAssertEqual(dictation.state, .idle, "the person starts again")
  }

  func testAnInterruptionWhileConnectingEndsTheSession() async throws {
    let silent = FakeRelay.replying(to: "stop", with: [#"{"type":"done"}"#])
    let source = RelayDictationSource(
      language: nil, polish: true, input: microphone, notifications: notifications
    ) { silent }

    dictation.start(source: source)
    try await eventually("the start message") { !silent.sent.isEmpty }
    XCTAssertEqual(dictation.state, .connecting)

    interruption(.began)
    await dictation.session?.value

    XCTAssertEqual(dictation.state, .idle)
    XCTAssertTrue(microphone.stopped)
  }

  func testANewInputRouteRestartsCaptureAndKeepsTheSession() async throws {
    try await startDrafting()
    let audioBefore = relay.sent.filter { $0.audio != nil }.count

    routeChange(.newDeviceAvailable)
    try await eventually("the new route's audio") {
      relay.sent.filter { $0.audio != nil }.count == audioBefore + microphone.rerouted.count
    }

    XCTAssertEqual(microphone.restarts, 1)
    XCTAssertEqual(dictation.state, .listening)
    XCTAssertEqual(view.editor.dictationDraft(), " world")

    dictation.stop()
    await assertFinishedWithFinalText()
  }

  func testAnEngineConfigurationChangeRestartsCapture() async throws {
    try await startDrafting()

    post(.AVAudioEngineConfigurationChange, object: NSObject())
    post(.AVAudioEngineConfigurationChange, object: microphone.engine)
    try await eventually("a restart") { microphone.restarts > 0 }
    try await Task.sleep(for: .milliseconds(50))

    XCTAssertEqual(microphone.restarts, 1, "only this input's engine counts")
    XCTAssertEqual(dictation.state, .listening)

    dictation.stop()
    await assertFinishedWithFinalText()
  }

  func testARouteWithNoMicrophoneFinishesTheSessionCleanly() async throws {
    microphone.restartFailure = .audio("the microphone has no input format")
    try await startDrafting()

    routeChange(.oldDeviceUnavailable)
    try await eventually("finishing") { dictation.state != .listening }

    await assertFinishedWithFinalText()
  }

  func testRouteChangesThatLeaveTheInputAloneAreIgnored() async throws {
    try await startDrafting()

    routeChange(.categoryChange)
    try await Task.sleep(for: .milliseconds(50))

    XCTAssertEqual(microphone.restarts, 0)
    XCTAssertEqual(dictation.state, .listening)

    dictation.stop()
    await assertFinishedWithFinalText()
  }

  func testNotificationsAfterTheSessionChangeNothing() async throws {
    try await startDrafting()
    dictation.stop()
    await assertFinishedWithFinalText()

    interruption(.began)
    routeChange(.newDeviceAvailable)
    try await Task.sleep(for: .milliseconds(50))

    XCTAssertEqual(dictation.state, .idle)
    XCTAssertEqual(microphone.restarts, 0)
  }
}
