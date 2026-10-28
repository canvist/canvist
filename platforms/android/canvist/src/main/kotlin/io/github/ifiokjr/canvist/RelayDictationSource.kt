package io.github.ifiokjr.canvist

import android.content.Context
import android.util.Log
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.channels.ClosedReceiveChannelException
import kotlinx.coroutines.channels.ReceiveChannel
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString.Companion.toByteString

/**
 * Dictation through canvist's relay (`services/dictation`): the microphone
 * streams to a speech model on your server, and drafts and final text stream
 * back.
 *
 * The relay is the source to reach for first. Hosted and self-hosted models
 * revise their drafts, punctuate, and take an optional language model's
 * polish far better than the device's recognizer.
 *
 * `token` is a short-lived token your app's server mints for the person
 * signed in; never ship the relay's secret or a provider's key in the app.
 * The app needs the `RECORD_AUDIO` permission, granted before starting, and
 * `INTERNET`.
 */
class RelayDictationSource : DictationSource {
    private val language: String?
    private val polish: Boolean
    private val capture: AudioCapture
    private val events: AudioEvents
    private val connect: () -> DictationTransport

    /**
     * Dictation through the relay at [url], such as
     * `wss://dictation.example.com/v1/dictation`, authorized by [token].
     * [language] is a BCP 47 tag, or `null` for the speech model to detect
     * it; [polish] asks for the relay's language-model cleanup when it has
     * one. Throws [IllegalArgumentException] for a URL that isn't `ws`,
     * `wss`, `http`, or `https`.
     */
    constructor(
        context: Context,
        url: String,
        token: String,
        language: String? = null,
        polish: Boolean = true,
    ) : this(
        language,
        polish,
        MicrophoneCapture(context.applicationContext),
        SystemAudioEvents(context.applicationContext),
        OkHttpTransport.connector(url, token),
    )

    /**
     * Dictation capturing from [capture], told about the system's audio by
     * [events], over the transports [connect] opens.
     */
    internal constructor(
        language: String?,
        polish: Boolean,
        capture: AudioCapture,
        events: AudioEvents,
        connect: () -> DictationTransport,
    ) {
        this.language = language
        this.polish = polish
        this.capture = capture
        this.events = events
        this.connect = connect
    }

    /**
     * Stops the microphone and ends the audio stream, so the stop message
     * follows the last audio sent. Set while a session runs.
     */
    private var endAudio: (() -> Unit)? = null

    /** Whether [finish] was called during this session, perhaps before the microphone started. */
    private var finishing = false

    override suspend fun run(
        editor: Editor,
        progress: (DictationProgress) -> Unit,
    ) {
        val audio = Channel<ByteArray>(Channel.UNLIMITED)

        try {
            // Capture before connecting: a refused microphone opens no socket,
            // and the first words wait in the channel while the relay connects.
            capture.start { chunk -> audio.trySend(chunk) }
            endAudio = {
                capture.stop()
                audio.close()
            }

            // Stopping before the microphone started stops it now.
            if (finishing) finish()

            // Losing audio focus or the microphone mid-session is handled as
            // stopping is, so the words already said still become final.
            val end = {
                progress(DictationProgress.FINISHING)
                finish()
            }

            CaptureSupervisor(events, capture::restart, end).use { stream(audio, editor, progress) }
        } finally {
            capture.stop()
            audio.close()
            endAudio = null
            finishing = false
        }
    }

    override fun finish() {
        finishing = true
        endAudio?.invoke()
    }

    /** Send the start message and the audio, and apply the relay's messages until it is done. */
    private suspend fun stream(
        audio: ReceiveChannel<ByteArray>,
        editor: Editor,
        progress: (DictationProgress) -> Unit,
    ) {
        val transport = connect()

        try {
            transport.send(editor.dictationStartMessage(language, polish))

            coroutineScope {
                val sender = launch { send(audio, transport) }
                receive(transport, editor, progress)

                // The session is over once the relay says so, whether or not the
                // audio was all sent: the relay may end it at its time limit.
                sender.cancel()
            }
        } finally {
            transport.close()
        }
    }

    /** Send every chunk of audio, then the stop message once the audio ends. */
    private suspend fun send(
        audio: ReceiveChannel<ByteArray>,
        transport: DictationTransport,
    ) {
        for (chunk in audio) transport.send(chunk)

        transport.send(dictationStopMessage())
    }

    /** Apply the relay's messages to [editor] until it says the session is done. */
    private suspend fun receive(
        transport: DictationTransport,
        editor: Editor,
        progress: (DictationProgress) -> Unit,
    ) {
        do {
            val event = apply(transport.receive(), editor)

            when (event) {
                DictationEvent.Ready -> progress(DictationProgress.LISTENING)
                is DictationEvent.Error -> report(event)
                is DictationEvent.Transcript, DictationEvent.Done -> Unit
            }
        } while (event != DictationEvent.Done)
    }

    private fun apply(
        message: String,
        editor: Editor,
    ): DictationEvent =
        try {
            editor.receiveDictation(message)
        } catch (error: CanvistException) {
            throw DictationException.Connection("the relay sent a message canvist can't read", error)
        }

    /** End the session on a fatal error; a non-fatal one, such as a failed polish, leaves the raw transcript final. */
    private fun report(error: DictationEvent.Error) {
        if (error.fatal) throw DictationException.Relay(error.code, error.message)

        Log.i(TAG, "the relay reported ${error.code}")
    }

    private companion object {
        const val TAG = "CanvistDictation"
    }
}

/** A WebSocket to the relay, or a scripted one in tests. */
internal interface DictationTransport {
    /** Queue a text message. Throws [DictationException.Connection] once the socket is closed. */
    fun send(text: String)

    /** Queue a binary message. Throws [DictationException.Connection] once the socket is closed. */
    fun send(audio: ByteArray)

    /**
     * The next text message. Throws [DictationException.Connection] once the
     * socket closes. The relay sends no binary messages.
     */
    suspend fun receive(): String

    /** Close the socket. Safe to call more than once. */
    fun close()
}

/** The relay's WebSocket over OkHttp, authorized with a bearer token. It connects as it is made. */
internal class OkHttpTransport(
    request: Request,
) : DictationTransport {
    private val messages = Channel<String>(Channel.UNLIMITED)
    private val socket = client.newWebSocket(request, Listener())

    override fun send(text: String) = queued(socket.send(text))

    override fun send(audio: ByteArray) = queued(socket.send(audio.toByteString()))

    private fun queued(sent: Boolean) {
        if (!sent) throw DictationException.Connection("closed")
    }

    override suspend fun receive(): String =
        try {
            messages.receive()
        } catch (error: ClosedReceiveChannelException) {
            throw DictationException.Connection("closed", error)
        }

    override fun close() {
        socket.close(NORMAL_CLOSURE, null)
        messages.close()
    }

    /** Hands text messages to [receive] in order, and ends it when the socket closes. */
    private inner class Listener : WebSocketListener() {
        override fun onMessage(
            webSocket: WebSocket,
            text: String,
        ) {
            messages.trySend(text)
        }

        override fun onClosing(
            webSocket: WebSocket,
            code: Int,
            reason: String,
        ) {
            webSocket.close(NORMAL_CLOSURE, null)
            messages.close()
        }

        override fun onFailure(
            webSocket: WebSocket,
            t: Throwable,
            response: Response?,
        ) {
            messages.close(DictationException.Connection(t.message ?: t.javaClass.simpleName, t))
        }
    }

    companion object {
        private const val NORMAL_CLOSURE = 1000

        /** One client for every session, as OkHttp recommends: they share its connections and threads. */
        private val client by lazy { OkHttpClient() }

        /**
         * Opens a socket to the relay at [url] with [token] as its bearer token
         * for each session. Throws [IllegalArgumentException] for a URL OkHttp
         * can't connect to, now rather than when a session starts.
         */
        fun connector(
            url: String,
            token: String,
        ): () -> DictationTransport {
            val request =
                Request
                    .Builder()
                    .url(url)
                    .header("Authorization", "Bearer $token")
                    .build()

            return { OkHttpTransport(request) }
        }
    }
}
