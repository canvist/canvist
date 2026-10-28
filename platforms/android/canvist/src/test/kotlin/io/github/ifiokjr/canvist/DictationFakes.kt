package io.github.ifiokjr.canvist

import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.channels.ClosedReceiveChannelException

/** An editor holding [markdown] with the caret at the end, measured without a platform. */
internal fun dictationEditor(markdown: String = "Hello"): Editor {
    val options = defaultEditorOptions().copy(extensions = StarterKit().extensions.names)
    val editor = Editor(options, FixedMeasurer())
    editor.load(DocFormat.MARKDOWN, markdown)
    val end = editor.textLength()
    editor.select(end, end)

    return editor
}

// What the relay says, as the tests script it.
internal const val READY = """{"type":"ready"}"""
internal const val DONE = """{"type":"done"}"""
internal const val PARTIAL_WORLD = """{"type":"partial","segment":0,"text":"world"}"""
internal const val FINAL_WORLD = """{"type":"final","segment":0,"text":"world."}"""

/** Measures every character [ADVANCE] dp wide, with a 12 dp ascent and 4 dp descent. */
private class FixedMeasurer : PlatformMeasurer {
    override fun measure(request: MeasureRequest): MeasureResponse =
        MeasureResponse(
            metrics = request.metrics.map { VerticalMetrics(ascent = ASCENT, descent = DESCENT) },
            advances = request.segments.flatMap { segment -> List(scalars(segment.text)) { ADVANCE } },
        )

    private companion object {
        fun scalars(text: String): Int = text.codePointCount(0, text.length)

        const val ADVANCE = 8f
        const val ASCENT = 12f
        const val DESCENT = 4f
    }
}

/** 100 ms of one sample over and over, as the relay reads it. */
private fun chunk(sample: Int): ByteArray {
    val samples = ShortArray(Pcm.CHUNK_SAMPLES) { sample.toShort() }

    return Pcm.littleEndian(samples, samples.size)
}

/** A microphone that delivers a few chunks when started, and a few more, as a new route would, when restarted. */
internal class FakeCapture : AudioCapture {
    val chunks = listOf(chunk(0), chunk(100), chunk(200))

    /** What a headset's microphone delivers after a restart. */
    val rerouted = listOf(chunk(-100), chunk(-200))

    var failure: DictationException? = null
    var restartFailure: DictationException? = null
    var restarts = 0
        private set
    var stopped = false
        private set
    private var receive: ((ByteArray) -> Unit)? = null

    override fun start(receive: (ByteArray) -> Unit) {
        failure?.let { throw it }

        this.receive = receive
        chunks.forEach(receive)
    }

    override fun restart() {
        restarts += 1
        restartFailure?.let { throw it }

        rerouted.forEach { chunk -> receive?.invoke(chunk) }
    }

    override fun stop() {
        stopped = true
        receive = null
    }
}

/** What a [FakeRelay] was sent. */
internal sealed interface Sent {
    data class Text(
        val text: String,
    ) : Sent

    class Audio(
        val bytes: ByteArray,
    ) : Sent
}

/**
 * A relay that records what it is sent and answers text messages with
 * scripted replies, keyed by the message's `type`.
 */
internal class FakeRelay(
    private val replies: (String) -> List<String>,
) : DictationTransport {
    val sent = mutableListOf<Sent>()
    var closed = false
        private set
    private val incoming = Channel<String>(Channel.UNLIMITED)

    override fun send(text: String) {
        if (closed) throw DictationException.Connection("closed")

        sent += Sent.Text(text)
        replies(type(text)).forEach { reply -> incoming.trySend(reply) }
    }

    override fun send(audio: ByteArray) {
        if (closed) throw DictationException.Connection("closed")

        sent += Sent.Audio(audio)
    }

    override suspend fun receive(): String =
        try {
            incoming.receive()
        } catch (error: ClosedReceiveChannelException) {
            throw DictationException.Connection("closed", error)
        }

    override fun close() {
        closed = true
        incoming.close()
    }

    /** Drop the connection, as the network would. */
    fun drop() = incoming.close()

    val texts get() = sent.filterIsInstance<Sent.Text>().map { it.text }
    val audio get() = sent.filterIsInstance<Sent.Audio>()

    private fun type(message: String): String =
        TYPE
            .find(message)
            ?.groupValues
            ?.get(1)
            .orEmpty()

    companion object {
        private val TYPE = Regex("\"type\":\"(\\w+)\"")

        /** A relay that answers the start message with [start], and the stop message with [stop]. */
        fun scripted(
            start: List<String>,
            stop: List<String>,
        ) = FakeRelay { type ->
            when (type) {
                "start" -> start
                "stop" -> stop
                else -> emptyList()
            }
        }

        /** A relay that answers only messages of [type], with [replies]. */
        fun replying(
            type: String,
            vararg replies: String,
        ) = FakeRelay { message -> if (message == type) replies.toList() else emptyList() }
    }
}

/** System audio notifications the test sends itself. */
internal class FakeAudioEvents : AudioEvents {
    private var listener: ((CaptureEvent) -> Unit)? = null
    var watching = false
        private set

    override fun watch(listener: (CaptureEvent) -> Unit): AutoCloseable {
        this.listener = listener
        watching = true

        return AutoCloseable { watching = false }
    }

    /** Report [event] as the system would, even after the session stopped watching. */
    fun emit(event: CaptureEvent) {
        listener?.invoke(event)
    }
}

/** A recognizer that replays scripted events: each [listen] takes the next utterance's script. */
internal class FakeRecognizer(
    private val utterances: MutableList<List<RecognitionEvent>>,
    /** What it reports when stopped. */
    private val whenStopped: List<RecognitionEvent> = emptyList(),
) : Recognizer,
    RecognizerFactory {
    var listens = 0
        private set
    var stops = 0
        private set
    var closed = false
        private set
    var failure: DictationException? = null
    private var report: ((RecognitionEvent) -> Unit)? = null

    override fun open(report: (RecognitionEvent) -> Unit): Recognizer {
        failure?.let { throw it }

        this.report = report

        return this
    }

    override fun listen() {
        listens += 1
        utterances.removeFirstOrNull()?.forEach { event -> report?.invoke(event) }
    }

    override fun stop() {
        stops += 1
        whenStopped.forEach { event -> report?.invoke(event) }
    }

    override fun close() {
        closed = true
    }
}
