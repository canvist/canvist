@preconcurrency import AVFAudio
import UIKit
import XCTest

@testable import Canvist

/// Dictation driven by synthetic audio and a fake relay that replays
/// scripted messages: no microphone or network.
@MainActor
final class DictationTests: XCTestCase {
  private func view(_ markdown: String = "Hello") throws -> CanvistView {
    try dictationView(markdown)
  }

  private func relaySource(_ relay: FakeRelay, microphone: FakeMicrophone = FakeMicrophone())
    -> RelayDictationSource
  {
    RelayDictationSource(language: "en", polish: true, input: microphone) { relay }
  }

  // MARK: Audio
  func testConvertsAudioTo16KilohertzLittleEndianPCM() throws {
    let converter = try PCMConverter.relay()
    let chunks = try (0..<10).map { index in
      let buffer = try XCTUnwrap(sine(seconds: 0.1, sampleRate: 48_000, offset: index))
      return PCMConverter.littleEndianBytes(try converter.convert(buffer))
    }

    XCTAssertEqual(converter.output.sampleRate, 16_000)
    XCTAssertEqual(converter.output.commonFormat, .pcmFormatInt16)
    XCTAssertEqual(converter.output.channelCount, 1)

    // 100 ms at 16 kHz is 1,600 samples, 3,200 bytes; the resampler holds
    // a few back at the start.
    for chunk in chunks.dropFirst() {
      XCTAssertEqual(chunk.count, 3_200, accuracy: 64)
    }

    let samples = Data(chunks.joined()).withUnsafeBytes { bytes in
      (0..<bytes.count / 2).map { index in
        Int16(littleEndian: bytes.loadUnaligned(fromByteOffset: index * 2, as: Int16.self))
      }
    }
    // One second of audio, less the resampler's latency (under 25 ms).
    XCTAssertLessThanOrEqual(samples.count, 16_000)
    XCTAssertGreaterThan(samples.count, 16_000 - 400)
    let peak = samples.map { abs(Int($0)) }.max() ?? 0
    XCTAssertEqual(Double(peak), Double(Int16.max) / 2, accuracy: 1_000, "a sine at half scale")
  }

  // MARK: Relay
  func testRelaySessionSendsStartAudioThenStopAndInsertsFinalText() async throws {
    let view = try view()
    let microphone = FakeMicrophone()
    let relay = FakeRelay { message in
      switch message {
      case "start": [#"{"type":"ready"}"#, #"{"type":"partial","segment":0,"text":"world"}"#]
      case "stop": [#"{"type":"final","segment":0,"text":"world."}"#, #"{"type":"done"}"#]
      default: []
      }
    }
    let dictation = CanvistDictation(view: view)

    dictation.start(source: relaySource(relay, microphone: microphone))
    XCTAssertEqual(dictation.state, .connecting)
    try await eventually("the draft") { view.editor.dictationDraft() == " world" }
    XCTAssertEqual(dictation.state, .listening)
    XCTAssertEqual(view.editor.plainText(), "Hello", "drafts stay out of the document")

    dictation.stop()
    XCTAssertEqual(dictation.state, .finishing)
    await dictation.session?.value

    XCTAssertEqual(dictation.state, .idle)
    XCTAssertEqual(view.editor.plainText(), "Hello world.")
    XCTAssertNil(view.editor.dictationDraft())
    XCTAssertTrue(microphone.stopped)

    let sent = relay.sent
    let start = try XCTUnwrap(sent.first?.text)
    let json = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(start.utf8)) as? [String: Any])
    XCTAssertEqual(json["type"] as? String, "start")
    XCTAssertEqual(json["context"] as? String, "Hello")
    XCTAssertEqual(json["language"] as? String, "en")
    XCTAssertEqual(sent.last?.text, #"{"type":"stop"}"#)
    let audio = sent.dropFirst().dropLast()
    XCTAssertEqual(audio.count, microphone.buffers.count, "one binary frame per buffer")
    XCTAssertTrue(audio.allSatisfy { $0.audio.map { $0.count > 0 && $0.count % 2 == 0 } ?? false })
  }

  func testAFatalRelayErrorFailsAndLeavesNoDraft() async throws {
    let view = try view()
    let relay = FakeRelay { message in
      guard message == "start" else { return [] }

      return [
        #"{"type":"ready"}"#,
        #"{"type":"partial","segment":0,"text":"world"}"#,
        #"{"type":"error","code":"provider","message":"upstream closed","fatal":true}"#,
      ]
    }
    let dictation = CanvistDictation(view: view)

    dictation.start(source: relaySource(relay))
    await dictation.session?.value

    XCTAssertEqual(dictation.state, .failed(.relay(.provider, message: "upstream closed")))
    XCTAssertNil(view.editor.dictationDraft())
    XCTAssertEqual(view.editor.plainText(), "Hello")
  }

  func testANonFatalRelayErrorKeepsListening() async throws {
    let view = try view()
    let relay = FakeRelay { message in
      switch message {
      case "start":
        [
          #"{"type":"ready"}"#,
          #"{"type":"error","code":"polish","message":"timed out","fatal":false}"#,
          #"{"type":"final","segment":0,"text":"world"}"#,
        ]
      case "stop": [#"{"type":"done"}"#]
      default: []
      }
    }
    let dictation = CanvistDictation(view: view)

    dictation.start(source: relaySource(relay))
    try await eventually("the final text") { view.editor.plainText() == "Hello world" }
    XCTAssertEqual(dictation.state, .listening)

    dictation.stop()
    await dictation.session?.value
    XCTAssertEqual(dictation.state, .idle)
  }

  func testStoppingMidDraftClearsIt() async throws {
    let view = try view()
    let relay = FakeRelay { message in
      switch message {
      case "start": [#"{"type":"ready"}"#, #"{"type":"partial","segment":0,"text":"world"}"#]
      case "stop": [#"{"type":"done"}"#]
      default: []
      }
    }
    let dictation = CanvistDictation(view: view)

    dictation.start(source: relaySource(relay))
    try await eventually("the draft") { view.editor.dictationDraft() != nil }
    dictation.stop()
    await dictation.session?.value

    XCTAssertEqual(dictation.state, .idle)
    XCTAssertNil(view.editor.dictationDraft())
    XCTAssertEqual(view.editor.plainText(), "Hello")
  }

  func testStoppingWhileConnectingEndsTheSession() async throws {
    let view = try view()
    let microphone = FakeMicrophone()
    let relay = FakeRelay.replying(to: "stop", with: [#"{"type":"done"}"#])
    let dictation = CanvistDictation(view: view)

    dictation.start(source: relaySource(relay, microphone: microphone))
    dictation.stop()
    try await eventually("the session to end") { dictation.state == .idle }

    XCTAssertTrue(microphone.stopped)
    XCTAssertEqual(relay.sent.last?.text, #"{"type":"stop"}"#)
  }

  func testCancellingMidDraftClearsIt() async throws {
    let view = try view()
    let microphone = FakeMicrophone()
    let relay = FakeRelay.replying(
      to: "start", with: [#"{"type":"ready"}"#, #"{"type":"partial","segment":0,"text":"world"}"#])
    let dictation = CanvistDictation(view: view)

    dictation.start(source: relaySource(relay, microphone: microphone))
    try await eventually("the draft") { view.editor.dictationDraft() != nil }
    dictation.cancel()
    await dictation.session?.value

    XCTAssertEqual(dictation.state, .idle)
    XCTAssertNil(view.editor.dictationDraft())
    XCTAssertTrue(microphone.stopped)
    XCTAssertTrue(relay.closed)
  }

  func testTheSocketClosingMidDraftFailsAndClearsIt() async throws {
    let view = try view()
    let relay = FakeRelay.replying(
      to: "start", with: [#"{"type":"ready"}"#, #"{"type":"partial","segment":0,"text":"world"}"#])
    let dictation = CanvistDictation(view: view)

    dictation.start(source: relaySource(relay))
    try await eventually("the draft") { view.editor.dictationDraft() != nil }
    relay.close()
    await dictation.session?.value

    XCTAssertEqual(dictation.state, .failed(.connection("closed")))
    XCTAssertNil(view.editor.dictationDraft())
  }

  func testARefusedMicrophoneFailsWithoutConnecting() async throws {
    let view = try view()
    let microphone = FakeMicrophone()
    microphone.failure = .microphoneDenied
    var connected = false
    let source = RelayDictationSource(language: nil, polish: true, input: microphone) {
      connected = true

      return FakeRelay.replying(to: "start", with: [])
    }
    let dictation = CanvistDictation(view: view)

    dictation.start(source: source)
    await dictation.session?.value

    XCTAssertEqual(dictation.state, .failed(.microphoneDenied))
    XCTAssertFalse(connected)
    XCTAssertNotNil(DictationError.microphoneDenied.errorDescription)
  }

  // MARK: On-device
  func testOnDeviceResultsBecomeSegments() throws {
    guard #available(iOS 26, *) else { throw XCTSkip("SpeechAnalyzer needs iOS 26") }

    let view = try view("Hi")
    let source = OnDeviceDictationSource(
      locale: Locale(identifier: "en-US"), input: FakeMicrophone())

    try source.dictate(" hello", isFinal: false, into: view.editor)
    XCTAssertEqual(view.editor.dictationDraft(), " hello")

    try source.dictate(" Hello.", isFinal: true, into: view.editor)
    try source.dictate(" again", isFinal: false, into: view.editor)

    XCTAssertEqual(view.editor.plainText(), "Hi Hello.")
    XCTAssertEqual(view.editor.dictationDraft(), " again", "the next segment's draft")
  }

  // MARK: Frames
  func testTheDisplayLinkRunsOnlyWhileFramesAnimate() throws {
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
    window.makeKeyAndVisible()
    defer { window.isHidden = true }

    let view = try view()
    view.frame = window.bounds
    window.addSubview(view)
    view.applyMotionPreference(reduced: false)
    view.layoutIfNeeded()
    XCTAssertNil(view.displayLink, "an idle editor asks for no frames")

    _ = try view.editor.dictate(segment: 0, text: "world", isFinal: false)
    view.layoutIfNeeded()
    XCTAssertNotNil(view.displayLink, "the draft breathes")

    _ = view.editor.stopDictation()
    view.layoutIfNeeded()
    XCTAssertNil(view.displayLink, "nothing is animating")

    _ = try view.editor.dictate(segment: 0, text: "world", isFinal: false)
    view.layoutIfNeeded()
    view.removeFromSuperview()
    XCTAssertNil(view.displayLink, "off screen")
  }

  func testReducedMotionTurnsAnimationsOff() throws {
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
    window.makeKeyAndVisible()
    defer { window.isHidden = true }

    let view = try view()
    view.frame = window.bounds
    window.addSubview(view)
    view.applyMotionPreference(reduced: true)

    _ = try view.editor.dictate(segment: 0, text: "world", isFinal: false)
    view.layoutIfNeeded()

    XCTAssertNil(view.displayLink)
    XCTAssertFalse(try view.editor.frame().animated)
    XCTAssertEqual(view.editor.dictationDraft(), " world", "the draft still shows, steadily")
  }
}
