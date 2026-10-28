package io.github.ifiokjr.canvist

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.channels.ReceiveChannel
import kotlinx.coroutines.withTimeoutOrNull
import java.util.Locale

/**
 * Dictation with the device's own recognizer, [SpeechRecognizer]: for
 * offline use, and for apps without a relay.
 *
 * It prefers the on-device recognizer (Android 12 and newer), so speech
 * stays on the phone, and falls back to the system's recognition service.
 * There is no polish pass, so fillers and spoken corrections stay in the
 * text. The recognizer stops after each pause; this source starts it again
 * until the session is stopped, each utterance becoming one segment. The
 * app needs the `RECORD_AUDIO` permission, granted before starting.
 */
class OnDeviceDictationSource : DictationSource {
    private val language: String?
    private val recognizers: RecognizerFactory
    private val events: AudioEvents

    /** Dictation in [language], a BCP 47 tag, or the device's language when `null`. */
    constructor(context: Context, language: String? = null) : this(
        language,
        SystemRecognizers(context.applicationContext, language),
        SystemAudioEvents(context.applicationContext),
    )

    /** Dictation from the recognizers [recognizers] opens, told about the system's audio by [events]. */
    internal constructor(language: String?, recognizers: RecognizerFactory, events: AudioEvents) {
        this.language = language
        this.recognizers = recognizers
        this.events = events
    }

    /** The recognizer listening now. Set while a session runs. */
    private var recognizer: Recognizer? = null

    /** What the recognizer reported, waiting for the session. Set while a session runs. */
    private var results: Channel<RecognitionEvent>? = null

    /** Whether [finish] was called during this session, perhaps before the recognizer started. */
    private var finishing = false

    /** The segment the recognizer is reporting now. */
    private var segment = 0u

    override suspend fun run(
        editor: Editor,
        progress: (DictationProgress) -> Unit,
    ) {
        val results = Channel<RecognitionEvent>(Channel.UNLIMITED)
        this.results = results
        segment = 0u

        try {
            val recognizer = recognizers.open { event -> results.trySend(event) }
            this.recognizer = recognizer
            recognizer.listen()

            // Stopping before the recognizer started stops it now.
            if (finishing) finish()

            // Losing audio focus is handled as stopping is. A new microphone
            // ends the utterance, and the next one listens on the new route.
            val end = {
                progress(DictationProgress.FINISHING)
                finish()
            }

            CaptureSupervisor(events, recognizer::stop, end).use { transcribe(results, editor, progress) }
        } finally {
            recognizer?.close()
            recognizer = null
            this.results = null
            finishing = false
        }
    }

    override fun finish() {
        finishing = true
        recognizer?.stop()
        // Wake the loop waiting for results, so it gives up on a recognizer
        // that says nothing more.
        results?.trySend(RecognitionEvent.Stopped)
    }

    /** Dictate the recognizer's results, listening again after each utterance, until finished. */
    private suspend fun transcribe(
        results: ReceiveChannel<RecognitionEvent>,
        editor: Editor,
        progress: (DictationProgress) -> Unit,
    ) {
        while (true) {
            val event = next(results) ?: return

            if (!handle(event, editor, progress)) return
        }
    }

    /** Act on what the recognizer reported. Returns whether the session goes on. */
    private fun handle(
        event: RecognitionEvent,
        editor: Editor,
        progress: (DictationProgress) -> Unit,
    ): Boolean {
        when (event) {
            RecognitionEvent.Ready -> progress(DictationProgress.LISTENING)
            RecognitionEvent.Stopped -> Unit
            is RecognitionEvent.Partial -> dictate(event.text, isFinal = false, editor)
            is RecognitionEvent.Final -> dictate(event.text, isFinal = true, editor)
            is RecognitionEvent.Failed -> throwFailure(event.code)
        }

        // An utterance ends with a final result or an error: listen for the next.
        val ended = event is RecognitionEvent.Final || event is RecognitionEvent.Failed

        return !ended || listenAgain()
    }

    /** Throw what a recognizer error means, unless it was silence or the session is finishing anyway. */
    private fun throwFailure(code: Int) {
        if (finishing) return

        failure(code, language)?.let { throw it }
    }

    /**
     * The recognizer's next event, or `null` when it said nothing for a while
     * after being stopped: some recognizers report nothing when stopped
     * between utterances.
     */
    private suspend fun next(results: ReceiveChannel<RecognitionEvent>): RecognitionEvent? {
        if (!finishing) return results.receive()

        return withTimeoutOrNull(FINISH_MILLIS) { results.receive() }
    }

    /** Listen for the next utterance unless finishing. Returns whether the session goes on. */
    private fun listenAgain(): Boolean {
        val recognizer = recognizer

        if (finishing || recognizer == null) return false

        recognizer.listen()

        return true
    }

    /**
     * Show or insert what the recognizer said. A partial result is the draft
     * of the current utterance, and a final one closes its segment. An
     * utterance that ended with no words clears its draft and keeps the
     * segment for the next one.
     */
    internal fun dictate(
        text: String,
        isFinal: Boolean,
        editor: Editor,
    ) {
        val words = text.trim()

        if (isFinal && words.isEmpty()) {
            editor.dictate(segment, "", false)

            return
        }

        editor.dictate(segment, words, isFinal)

        if (isFinal) segment += 1u
    }

    companion object {
        private const val FINISH_MILLIS = 3_000L

        /** Whether this device has a speech recognizer: on the device, or the system's service. */
        fun isAvailable(context: Context): Boolean {
            if (SystemRecognizers.onDevice(context)) return true

            return SpeechRecognizer.isRecognitionAvailable(context)
        }

        /**
         * What a recognizer error means for the session: `null` for silence,
         * after which the recognizer listens again, and the failure otherwise.
         */
        internal fun failure(
            code: Int,
            language: String?,
        ): DictationException? {
            val reason = "recognizer error $code"
            val tag = language ?: Locale.getDefault().toLanguageTag()

            return when (code) {
                SpeechRecognizer.ERROR_NO_MATCH, SpeechRecognizer.ERROR_SPEECH_TIMEOUT -> null
                SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS -> DictationException.MicrophoneDenied()
                SpeechRecognizer.ERROR_AUDIO -> DictationException.Audio(reason)
                ERROR_LANGUAGE_NOT_SUPPORTED -> DictationException.UnsupportedLanguage(tag)
                in CONNECTION_ERRORS -> DictationException.Connection(reason)
                else -> DictationException.RecognizerUnavailable(reason)
            }
        }

        // Android 12's codes, spelled out so the mapping compiles the same for every API level.
        private const val ERROR_TOO_MANY_REQUESTS = 10
        private const val ERROR_SERVER_DISCONNECTED = 11
        private const val ERROR_LANGUAGE_NOT_SUPPORTED = 12

        private val CONNECTION_ERRORS =
            setOf(
                SpeechRecognizer.ERROR_NETWORK,
                SpeechRecognizer.ERROR_NETWORK_TIMEOUT,
                SpeechRecognizer.ERROR_SERVER,
                ERROR_TOO_MANY_REQUESTS,
                ERROR_SERVER_DISCONNECTED,
            )
    }
}

/** What a speech recognizer reported. */
internal sealed interface RecognitionEvent {
    /** It is listening. */
    data object Ready : RecognitionEvent

    /** The session asked it to stop: what it heard follows, if anything. */
    data object Stopped : RecognitionEvent

    /** Its best guess at the utterance so far. */
    data class Partial(
        val text: String,
    ) : RecognitionEvent

    /** The utterance is over, and this is what was said. */
    data class Final(
        val text: String,
    ) : RecognitionEvent

    /** It stopped with a [SpeechRecognizer] error code. */
    data class Failed(
        val code: Int,
    ) : RecognitionEvent
}

/** A speech recognizer for one session: the system's, or a scripted one in tests. */
internal interface Recognizer : AutoCloseable {
    /** Listen for one utterance. */
    fun listen()

    /** Stop listening, and report what was heard as final. */
    fun stop()

    /** Stop at once and release the recognizer. */
    override fun close()
}

/** Opens a [Recognizer] for each session. */
internal fun interface RecognizerFactory {
    /**
     * A recognizer reporting to [report] on the main thread. Throws
     * [DictationException.MicrophoneDenied] without `RECORD_AUDIO`, and
     * [DictationException.RecognizerUnavailable] when the device has none.
     */
    fun open(report: (RecognitionEvent) -> Unit): Recognizer
}

/** The device's [SpeechRecognizer]s, on the device where it has one. */
internal class SystemRecognizers(
    private val context: Context,
    language: String?,
) : RecognizerFactory {
    private val intent =
        Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH)
            .putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
            .putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, true)
            .putExtra(RecognizerIntent.EXTRA_PREFER_OFFLINE, true)
            .apply { if (language != null) putExtra(RecognizerIntent.EXTRA_LANGUAGE, language) }

    override fun open(report: (RecognitionEvent) -> Unit): Recognizer {
        val permission = context.checkSelfPermission(Manifest.permission.RECORD_AUDIO)

        if (permission != PackageManager.PERMISSION_GRANTED) throw DictationException.MicrophoneDenied()

        val recognizer = create()
        recognizer.setRecognitionListener(Listener(report))

        return SystemRecognizer(recognizer, intent)
    }

    /** The on-device recognizer where there is one, else the system's recognition service. */
    private fun create(): SpeechRecognizer {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S && onDevice(context)) {
            return SpeechRecognizer.createOnDeviceSpeechRecognizer(context)
        }

        if (SpeechRecognizer.isRecognitionAvailable(context)) return SpeechRecognizer.createSpeechRecognizer(context)

        throw DictationException.RecognizerUnavailable("no recognition service")
    }

    companion object {
        /** Whether the device recognizes speech on the device itself. */
        fun onDevice(context: Context): Boolean =
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.S && SpeechRecognizer.isOnDeviceRecognitionAvailable(context)
    }
}

private class SystemRecognizer(
    private val recognizer: SpeechRecognizer,
    private val intent: Intent,
) : Recognizer {
    override fun listen() = recognizer.startListening(intent)

    override fun stop() = recognizer.stopListening()

    override fun close() {
        recognizer.cancel()
        recognizer.destroy()
    }
}

/** Turns [SpeechRecognizer]'s callbacks into [RecognitionEvent]s. */
private class Listener(
    private val report: (RecognitionEvent) -> Unit,
) : RecognitionListener {
    override fun onReadyForSpeech(params: Bundle?) = report(RecognitionEvent.Ready)

    override fun onPartialResults(partialResults: Bundle?) = report(RecognitionEvent.Partial(best(partialResults)))

    override fun onResults(results: Bundle?) = report(RecognitionEvent.Final(best(results)))

    override fun onError(error: Int) = report(RecognitionEvent.Failed(error))

    override fun onBeginningOfSpeech() = Unit

    override fun onRmsChanged(rmsdB: Float) = Unit

    override fun onBufferReceived(buffer: ByteArray?) = Unit

    override fun onEndOfSpeech() = Unit

    override fun onEvent(
        eventType: Int,
        params: Bundle?,
    ) = Unit

    /** The recognizer's most likely transcript. */
    private fun best(results: Bundle?): String =
        results
            ?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
            ?.firstOrNull()
            .orEmpty()
}
