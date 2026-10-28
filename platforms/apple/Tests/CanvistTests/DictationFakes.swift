@preconcurrency import AVFAudio
import UIKit
import XCTest

@testable import Canvist

/// A view holding `markdown`, with the caret at the end.
@MainActor
func dictationView(_ markdown: String = "Hello") throws -> CanvistView {
  let view = try CanvistView(extensions: StarterKit().extensions)
  try view.editor.load(format: .markdown, content: markdown)
  let end = view.editor.textLength()
  _ = try view.editor.select(anchor: end, head: end)

  return view
}

/// Wait for `condition`, failing after a couple of seconds.
@MainActor
func eventually(
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

/// A sine wave at half scale, as the microphone would deliver it: 32-bit
/// floats, the same in every channel. `offset` continues the wave from
/// earlier buffers.
func sine(
  seconds: Double, sampleRate: Double, channels: AVAudioChannelCount = 1, offset: Int = 0
) -> AVAudioPCMBuffer? {
  let frames = AVAudioFrameCount(seconds * sampleRate)

  guard let format = AVAudioFormat(standardFormatWithSampleRate: sampleRate, channels: channels),
    let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: frames),
    let data = buffer.floatChannelData
  else { return nil }

  buffer.frameLength = frames
  let start = offset * Int(frames)

  for channel in 0..<Int(channels) {
    for index in 0..<Int(frames) {
      data[channel][index] = 0.5 * sin(2 * .pi * 440 * Float(start + index) / Float(sampleRate))
    }
  }

  return buffer
}

/// A microphone that delivers a few synthetic buffers when started, and a
/// few in another format, as a new route would, when restarted.
@MainActor
final class FakeMicrophone: AudioInput {
  let engine: AnyObject = NSObject()
  let buffers = (0..<3).compactMap { index in sine(seconds: 0.1, sampleRate: 48_000, offset: index)
  }
  /// What a headset's microphone delivers after a restart.
  let rerouted = (0..<2).compactMap { index in
    sine(seconds: 0.1, sampleRate: 44_100, channels: 2, offset: index)
  }
  var failure: DictationError?
  var restartFailure: DictationError?
  private(set) var restarts = 0
  private(set) var stopped = false
  private var receive: (@Sendable (AVAudioPCMBuffer) -> Void)?

  func start(_ receive: @escaping @Sendable (AVAudioPCMBuffer) -> Void) async throws {
    if let failure {
      throw failure
    }

    self.receive = receive
    buffers.forEach(receive)
  }

  func restart() throws {
    restarts += 1

    if let restartFailure {
      throw restartFailure
    }

    rerouted.forEach { buffer in receive?(buffer) }
  }

  func stop() {
    stopped = true
    receive = nil
  }
}

/// A relay that records what it is sent and answers text messages with
/// scripted replies, keyed by the message's `type`.
final class FakeRelay: DictationTransport, @unchecked Sendable {
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

  /// A relay that answers only messages of `type`, with `replies`.
  static func replying(to type: String, with replies: [String]) -> FakeRelay {
    FakeRelay { message in
      guard message == type else { return [] }

      return replies
    }
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
  var text: String? {
    guard case .string(let text) = self else { return nil }

    return text
  }

  var audio: Data? {
    guard case .data(let data) = self else { return nil }

    return data
  }
}
