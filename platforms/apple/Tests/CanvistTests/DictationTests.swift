@preconcurrency import AVFAudio
import UIKit
import XCTest

@testable import Canvist

/// Dictation driven by synthetic audio and a fake relay that replays
/// scripted messages: no microphone or network.
@MainActor
final class DictationTests: XCTestCase {
  private let kit = StarterKit()

  /// A view holding `markdown`, with the caret at the end.
  private func view(_ markdown: String = "Hello") throws -> CanvistView {
    let view = try CanvistView(extensions: kit.extensions)
    try view.editor.load(format: .markdown, content: markdown)
    let end = view.editor.textLength()
    _ = try view.editor.select(anchor: end, head: end)

    return view
  }

  private func relaySource(_ relay: FakeRelay, microphone: FakeMicrophone = FakeMicrophone())
    -> RelayDictationSource
  {
    RelayDictationSource(language: "en", polish: true, input: microphone) { relay }
  }

  /// Wait for `condition`, failing after a couple of seconds.
  private func eventually(
    _ description: String, file: StaticString = #filePath, line: UInt = #line,
    _ condition: () -> Bool
  ) async throws {
    let deadline = Date.now.addingTimeInterval(2)

    while !condition() {
      guard Date.now < deadline else {
        XCTFail("timed out waiting for \(description)", file: file, line: line)

        return
      }

      try await Task.sleep(for: .milliseconds(5))
    }
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
    let relay = FakeRelay { message in message == "stop" ? [#"{"type":"done"}"#] : [] }
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
    let relay = FakeRelay { message in
      message == "start" ? [#"{"type":"ready"}"#, #"{"type":"partial","segment":0,"text":"world"}"#] : []
    }
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
    let relay = FakeRelay { message in
      message == "start" ? [#"{"type":"ready"}"#, #"{"type":"partial","segment":0,"text":"world"}"#] : []
    }
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

      return FakeRelay { _ in [] }
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
    let source = OnDeviceDictationSource(locale: Locale(identifier: "en-US"), input: FakeMicrophone())

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

/// A sine wave at half scale, as the microphone would deliver it: mono
/// 32-bit floats. `offset` continues the wave from earlier buffers.
private func sine(seconds: Double, sampleRate: Double, offset: Int = 0) -> AVAudioPCMBuffer? {
  let frames = AVAudioFrameCount(seconds * sampleRate)

  guard let format = AVAudioFormat(standardFormatWithSampleRate: sampleRate, channels: 1),
    let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: frames),
    let channel = buffer.floatChannelData?[0]
  else { return nil }

  buffer.frameLength = frames
  let start = offset * Int(frames)

  for index in 0..<Int(frames) {
    channel[index] = 0.5 * sin(2 * .pi * 440 * Float(start + index) / Float(sampleRate))
  }

  return buffer
}

/// A microphone that delivers a few synthetic buffers when started.
@MainActor
private final class FakeMicrophone: AudioInput {
  let buffers = (0..<3).compactMap { index in sine(seconds: 0.1, sampleRate: 48_000, offset: index) }
  var failure: DictationError?
  private(set) var stopped = false

  func start(_ receive: @escaping @Sendable (AVAudioPCMBuffer) -> Void) async throws {
    if let failure {
      throw failure
    }

    buffers.forEach(receive)
  }

  func stop() {
    stopped = true
  }
}

/// A relay that records what it is sent and answers text messages with
/// scripted replies, keyed by the message's `type`.
private final class FakeRelay: DictationTransport, @unchecked Sendable {
  private let lock = NSLock()
  private let replies: @Sendable (String) -> [String]
  private let incoming: AsyncStream<URLSessionWebSocketTask.Message>.Continuation
  private var messages: AsyncStream<URLSessionWebSocketTask.Message>.Iterator
  private var recorded: [URLSessionWebSocketTask.Message] = []
  private var isClosed = false

  init(replies: @escaping @Sendable (String) -> [String]) {
    let (stream, continuation) = AsyncStream.makeStream(of: URLSessionWebSocketTask.Message.self)
    self.replies = replies
    incoming = continuation
    messages = stream.makeAsyncIterator()
  }

  var sent: [URLSessionWebSocketTask.Message] {
    lock.withLock { recorded }
  }

  var closed: Bool {
    lock.withLock { isClosed }
  }

  func send(_ message: URLSessionWebSocketTask.Message) async throws {
    lock.withLock { recorded.append(message) }

    guard let text = message.text,
      let json = try JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any],
      let type = json["type"] as? String
    else { return }

    for reply in replies(type) {
      incoming.yield(.string(reply))
    }
  }

  func receive() async throws -> URLSessionWebSocketTask.Message {
    // One reader at a time, as the relay source reads.
    guard let message = await messages.next() else {
      try Task.checkCancellation()

      throw DictationError.connection("closed")
    }

    return message
  }

  func close() {
    lock.withLock { isClosed = true }
    incoming.finish()
  }
}

extension URLSessionWebSocketTask.Message {
  fileprivate var text: String? {
    guard case .string(let text) = self else { return nil }

    return text
  }

  fileprivate var audio: Data? {
    guard case .data(let data) = self else { return nil }

    return data
  }
}
