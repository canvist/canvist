package io.github.ifiokjr.canvist

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioDeviceCallback
import android.media.AudioDeviceInfo
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log

/**
 * Keeps a capture going through the system's audio changes while a dictation
 * session runs.
 *
 * - Losing audio focus (a phone call, or another app recording) ends the
 *   session as stopping does: the words already said become final. It does
 *   not resume when focus comes back; the person starts again.
 * - A microphone connecting or disconnecting (a headset) restarts capture on
 *   the new route, and ends the session as stopping does when it cannot.
 */
internal class CaptureSupervisor(
    events: AudioEvents,
    private val restart: () -> Unit,
    private val end: () -> Unit,
) : AutoCloseable {
    /** Whether events still matter: false once the session is over. */
    private var watching = true
    private val watch = events.watch(::handle)

    /** Stop watching, and give up audio focus. */
    override fun close() {
        watching = false
        watch.close()
    }

    /** Finish on an interruption; restart on an input change, finishing when capture can't restart. */
    private fun handle(event: CaptureEvent) {
        if (!watching) return

        if (event == CaptureEvent.INTERRUPTED) {
            Log.i(TAG, "audio focus was lost: finishing dictation")
            end()

            return
        }

        try {
            restart()
        } catch (error: DictationException) {
            Log.e(TAG, "restarting capture failed: ${error.message}")
            end()
        }
    }

    companion object {
        private const val TAG = "CanvistDictation"

        /**
         * Whether a focus change interrupts capture: losing focus for good or
         * for a while does, and being asked to duck playback doesn't.
         */
        fun interrupts(focusChange: Int): Boolean =
            focusChange == AudioManager.AUDIOFOCUS_LOSS || focusChange == AudioManager.AUDIOFOCUS_LOSS_TRANSIENT
    }
}

/** What a system notification means for a capture. */
internal enum class CaptureEvent {
    /** Audio focus was taken away. */
    INTERRUPTED,

    /** The set of microphones changed. */
    INPUT_CHANGED,
}

/** The system's audio notifications while capturing, or scripted ones in tests. */
internal fun interface AudioEvents {
    /**
     * Hold audio focus and report [CaptureEvent]s to [listener] on the main
     * thread until the returned handle is closed.
     */
    fun watch(listener: (CaptureEvent) -> Unit): AutoCloseable
}

/** Audio focus and input devices from [AudioManager]. */
internal class SystemAudioEvents(
    context: Context,
) : AudioEvents {
    private val audio = context.getSystemService(AudioManager::class.java)

    override fun watch(listener: (CaptureEvent) -> Unit): AutoCloseable {
        val main = Handler(Looper.getMainLooper())
        val focus = AudioFocus(audio, main) { listener(CaptureEvent.INTERRUPTED) }
        val inputs = InputDevices(audio, main) { listener(CaptureEvent.INPUT_CHANGED) }

        return AutoCloseable {
            focus.close()
            inputs.close()
        }
    }
}

/**
 * Exclusive transient audio focus, as speech recognition asks for: it pauses
 * playback while the microphone listens. A refused request (a call in
 * progress) is reported as an interruption.
 */
private class AudioFocus(
    private val audio: AudioManager,
    handler: Handler,
    interrupted: () -> Unit,
) : AutoCloseable {
    private val listener =
        AudioManager.OnAudioFocusChangeListener { change ->
            if (CaptureSupervisor.interrupts(change)) interrupted()
        }

    private val request =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            AudioFocusRequest
                .Builder(AudioManager.AUDIOFOCUS_GAIN_TRANSIENT_EXCLUSIVE)
                .setAudioAttributes(ATTRIBUTES)
                .setOnAudioFocusChangeListener(listener, handler)
                .build()
        } else {
            null
        }

    init {
        if (requestFocus() != AudioManager.AUDIOFOCUS_REQUEST_GRANTED) handler.post(interrupted)
    }

    private fun requestFocus(): Int {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && request != null) return audio.requestAudioFocus(request)

        val gain = AudioManager.AUDIOFOCUS_GAIN_TRANSIENT_EXCLUSIVE

        @Suppress("DEPRECATION")
        return audio.requestAudioFocus(listener, AudioManager.STREAM_MUSIC, gain)
    }

    override fun close() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && request != null) {
            audio.abandonAudioFocusRequest(request)

            return
        }

        @Suppress("DEPRECATION")
        audio.abandonAudioFocus(listener)
    }

    private companion object {
        val ATTRIBUTES: AudioAttributes =
            AudioAttributes
                .Builder()
                .setUsage(AudioAttributes.USAGE_ASSISTANT)
                .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                .build()
    }
}

/**
 * Reports when the set of microphones changes. Registering reports every
 * device already connected, so it compares against the set it started with.
 */
private class InputDevices(
    private val audio: AudioManager,
    handler: Handler,
    private val changed: () -> Unit,
) : AutoCloseable {
    private var inputs = inputIds()
    private val callback =
        object : AudioDeviceCallback() {
            override fun onAudioDevicesAdded(addedDevices: Array<out AudioDeviceInfo>) = check()

            override fun onAudioDevicesRemoved(removedDevices: Array<out AudioDeviceInfo>) = check()
        }

    init {
        audio.registerAudioDeviceCallback(callback, handler)
    }

    private fun check() {
        val now = inputIds()

        if (now == inputs) return

        inputs = now
        changed()
    }

    private fun inputIds(): Set<Int> = audio.getDevices(AudioManager.GET_DEVICES_INPUTS).map { it.id }.toSet()

    override fun close() = audio.unregisterAudioDeviceCallback(callback)
}
