package io.github.ifiokjr.canvist

import android.Manifest
import android.annotation.SuppressLint
import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.util.Log
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.concurrent.thread
import kotlin.math.max

/** Where speech audio comes from: the microphone, or scripted audio in tests. */
internal interface AudioCapture {
    /**
     * Start capturing, handing each chunk to [receive] as 16 kHz mono 16-bit
     * little-endian PCM, in the order captured, on a capture thread. Throws
     * [DictationException.MicrophoneDenied] without `RECORD_AUDIO`, and
     * [DictationException.Audio] when the microphone can't start.
     */
    fun start(receive: (ByteArray) -> Unit)

    /**
     * Capture from the current input route, after it changed, handing chunks
     * to the same receiver. Does nothing when not capturing.
     */
    fun restart()

    /** Stop capturing: once it returns, no more chunks arrive. Safe to call when not capturing. */
    fun stop()
}

/** The device's microphone, through [AudioRecord] at the rate the relay reads. */
internal class MicrophoneCapture(
    private val context: Context,
) : AudioCapture {
    private var receive: ((ByteArray) -> Unit)? = null
    private var recording: Recording? = null

    override fun start(receive: (ByteArray) -> Unit) {
        val permission = context.checkSelfPermission(Manifest.permission.RECORD_AUDIO)

        if (permission != PackageManager.PERMISSION_GRANTED) throw DictationException.MicrophoneDenied()

        recording = Recording.open(receive)
        this.receive = receive
    }

    override fun restart() {
        val receive = receive ?: return

        recording?.close()
        recording = null
        recording = Recording.open(receive)
    }

    override fun stop() {
        receive = null
        recording?.close()
        recording = null
    }
}

/** One [AudioRecord] and the thread that reads it. */
private class Recording(
    private val record: AudioRecord,
    receive: (ByteArray) -> Unit,
) : AutoCloseable {
    @Volatile private var open = true
    private val reader = thread(name = "canvist-dictation-audio") { read(receive) }

    private fun read(receive: (ByteArray) -> Unit) {
        val samples = ShortArray(Pcm.CHUNK_SAMPLES)

        while (open) {
            val count = record.read(samples, 0, samples.size)

            // The record died, as when its device goes away: the route change
            // that follows starts a new one.
            if (count < 0) {
                Log.e(TAG, "reading the microphone failed: $count")

                return
            }

            if (count > 0 && open) receive(Pcm.littleEndian(samples, count))
        }
    }

    override fun close() {
        open = false
        // Stopping returns a blocked read.
        record.stop()
        reader.join(JOIN_MILLIS)
        record.release()
    }

    companion object {
        private const val TAG = "CanvistDictation"
        private const val JOIN_MILLIS = 500L

        /** Prefer the source tuned for speech; not every device has it. */
        private val SOURCES = listOf(MediaRecorder.AudioSource.VOICE_RECOGNITION, MediaRecorder.AudioSource.MIC)

        /** Start recording from the current input route. */
        fun open(receive: (ByteArray) -> Unit): Recording {
            val minimum = AudioRecord.getMinBufferSize(Pcm.SAMPLE_RATE, CHANNEL, ENCODING)

            if (minimum <= 0) throw DictationException.Audio("16 kHz mono PCM is unsupported")

            val size = max(minimum, Pcm.CHUNK_SAMPLES * Pcm.BYTES_PER_SAMPLE * 2)
            val record =
                SOURCES.firstNotNullOfOrNull { source -> initialized(source, size) }
                    ?: throw DictationException.Audio("no audio source would initialize")

            start(record)

            return Recording(record, receive)
        }

        /** Start [record], releasing it when it won't. */
        private fun start(record: AudioRecord) {
            try {
                record.startRecording()
            } catch (error: IllegalStateException) {
                record.release()
                throw DictationException.Audio("the microphone would not start", error)
            }

            if (record.recordingState == AudioRecord.RECORDSTATE_RECORDING) return

            record.release()
            throw DictationException.Audio("another app is using the microphone")
        }

        // `MicrophoneCapture.start` checks `RECORD_AUDIO` before any recording opens.
        @SuppressLint("MissingPermission")
        private fun initialized(
            source: Int,
            size: Int,
        ): AudioRecord? {
            val record = AudioRecord(source, Pcm.SAMPLE_RATE, CHANNEL, ENCODING, size)

            if (record.state == AudioRecord.STATE_INITIALIZED) return record

            Log.w(TAG, "audio source $source did not initialize")
            record.release()

            return null
        }

        private const val CHANNEL = AudioFormat.CHANNEL_IN_MONO
        private const val ENCODING = AudioFormat.ENCODING_PCM_16BIT
    }
}

/** The audio format canvist's relay reads: 16 kHz mono 16-bit little-endian PCM. */
internal object Pcm {
    const val SAMPLE_RATE = 16_000
    const val BYTES_PER_SAMPLE = 2

    /**
     * Samples in each chunk captured: 100 ms. The relay accepts any length;
     * shorter means drafts arrive sooner.
     */
    const val CHUNK_SAMPLES = SAMPLE_RATE / 10

    /** The first [count] of [samples] as the relay's little-endian bytes. */
    fun littleEndian(
        samples: ShortArray,
        count: Int,
    ): ByteArray {
        val bytes = ByteBuffer.allocate(count * BYTES_PER_SAMPLE).order(ByteOrder.LITTLE_ENDIAN)
        bytes.asShortBuffer().put(samples, 0, count)

        return bytes.array()
    }
}
