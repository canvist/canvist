@preconcurrency import AVFAudio
import os

/// Where speech audio comes from: the microphone, or synthetic audio in
/// tests.
@MainActor
protocol AudioInput: AnyObject {
  /// Start capturing, calling `receive` with each buffer, in the order
  /// captured, on an audio thread.
  func start(_ receive: @escaping @Sendable (AVAudioPCMBuffer) -> Void) async throws

  /// Stop capturing. Safe to call when not capturing.
  func stop()
}

/// The device's microphone, through `AVAudioEngine`. Asks for permission
/// first, and holds the audio session only while capturing.
@MainActor
final class MicrophoneInput: AudioInput {
  private let engine = AVAudioEngine()
  private var capturing = false
  private let logger = Logger(subsystem: "dev.canvist", category: "Dictation")

  func start(_ receive: @escaping @Sendable (AVAudioPCMBuffer) -> Void) async throws {
    guard await AVAudioApplication.requestRecordPermission() else {
      throw DictationError.microphoneDenied
    }

    do {
      try capture(receive)
    } catch {
      stop()

      throw error as? DictationError ?? .audio(error.localizedDescription)
    }
  }

  private func capture(_ receive: @escaping @Sendable (AVAudioPCMBuffer) -> Void) throws {
    let session = AVAudioSession.sharedInstance()
    try session.setCategory(.record, mode: .measurement)
    try session.setActive(true)
    capturing = true

    let input = engine.inputNode
    let format = input.outputFormat(forBus: 0)

    // Installing a tap in a format with no rate raises an exception.
    guard format.sampleRate > 0, format.channelCount > 0 else {
      throw DictationError.audio("the microphone has no input format")
    }

    Self.tap(input, format: format, receive: receive)
    engine.prepare()
    try engine.start()
  }

  func stop() {
    guard capturing else { return }

    capturing = false
    engine.inputNode.removeTap(onBus: 0)
    engine.stop()

    do {
      try AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    } catch {
      logger.error("deactivating the audio session failed: \(error.localizedDescription, privacy: .public)")
    }
  }

  /// Install the tap outside the main actor: the engine calls it on an
  /// audio thread, and a closure made on the main actor would assert it
  /// runs there.
  private nonisolated static func tap(
    _ input: AVAudioInputNode,
    format: AVAudioFormat,
    receive: @escaping @Sendable (AVAudioPCMBuffer) -> Void
  ) {
    let frames = AVAudioFrameCount(format.sampleRate * tapSeconds)
    input.installTap(onBus: 0, bufferSize: frames, format: format) { buffer, _ in receive(buffer) }
  }

  /// How much audio each tap delivers. The relay accepts any length;
  /// shorter means drafts arrive sooner.
  private nonisolated static let tapSeconds = 0.1
}

/// Converts captured audio to the format a speech model reads, keeping the
/// resampler's state between buffers so a stream converts seamlessly. Use
/// one converter per stream, from one thread at a time.
final class PCMConverter: @unchecked Sendable {
  let output: AVAudioFormat
  private var converter: AVAudioConverter?

  init(output: AVAudioFormat) {
    self.output = output
  }

  /// A converter to the format canvist's relay reads: 16 kHz mono 16-bit
  /// integers.
  static func relay() throws(DictationError) -> PCMConverter {
    guard
      let format = AVAudioFormat(
        commonFormat: .pcmFormatInt16, sampleRate: 16_000, channels: 1, interleaved: true)
    else {
      throw .audio("16 kHz PCM is not a valid format")
    }

    return PCMConverter(output: format)
  }

  /// `buffer` in ``output``'s format. A resampler holds some audio back,
  /// so the result can be a little shorter than `buffer`'s duration; the
  /// rest comes with the next buffer.
  func convert(_ buffer: AVAudioPCMBuffer) throws(DictationError) -> AVAudioPCMBuffer {
    let converter = try converter(from: buffer.format)
    let ratio = output.sampleRate / buffer.format.sampleRate
    let capacity = AVAudioFrameCount((Double(buffer.frameLength) * ratio).rounded(.up)) + 1

    guard let converted = AVAudioPCMBuffer(pcmFormat: output, frameCapacity: capacity) else {
      throw .audio("could not allocate a converted buffer")
    }

    var supplied = false
    var failure: NSError?
    let status = converter.convert(to: converted, error: &failure) { _, inputStatus in
      // Hand over this buffer once, then wait for the next.
      if supplied {
        inputStatus.pointee = .noDataNow

        return nil
      }

      supplied = true
      inputStatus.pointee = .haveData

      return buffer
    }

    if status == .error {
      throw .audio(failure?.localizedDescription ?? "audio conversion failed")
    }

    return converted
  }

  /// A receiver for ``AudioInput/start(_:)`` that converts each buffer and
  /// hands it to `deliver`. A buffer that fails to convert is logged and
  /// skipped: the next one may well convert, and a gap is better than
  /// ending the session.
  func converting(
    _ deliver: @escaping @Sendable (AVAudioPCMBuffer) -> Void
  ) -> @Sendable (AVAudioPCMBuffer) -> Void {
    { buffer in self.convert(buffer, then: deliver) }
  }

  private func convert(_ buffer: AVAudioPCMBuffer, then deliver: (AVAudioPCMBuffer) -> Void) {
    let converted: AVAudioPCMBuffer

    do throws(DictationError) {
      converted = try convert(buffer)
    } catch {
      Self.logger.error("converting audio failed: \(error.logDescription, privacy: .public)")

      return
    }

    // The resampler can hold all of a short buffer back.
    if converted.frameLength > 0 {
      deliver(converted)
    }
  }

  private static let logger = Logger(subsystem: "dev.canvist", category: "Dictation")

  private func converter(from input: AVAudioFormat) throws(DictationError) -> AVAudioConverter {
    if let converter, converter.inputFormat == input {
      return converter
    }

    guard let converter = AVAudioConverter(from: input, to: output) else {
      throw .audio("cannot convert \(input) to \(output)")
    }

    self.converter = converter

    return converter
  }

  /// A buffer of 16-bit samples as little-endian bytes, as the relay reads
  /// them.
  static func littleEndianBytes(_ buffer: AVAudioPCMBuffer) -> Data {
    guard let channel = buffer.int16ChannelData?[0] else { return Data() }

    let samples = UnsafeBufferPointer(start: channel, count: Int(buffer.frameLength))
    var data = Data(capacity: samples.count * MemoryLayout<Int16>.size)

    for sample in samples {
      withUnsafeBytes(of: sample.littleEndian) { bytes in data.append(contentsOf: bytes) }
    }

    return data
  }
}
