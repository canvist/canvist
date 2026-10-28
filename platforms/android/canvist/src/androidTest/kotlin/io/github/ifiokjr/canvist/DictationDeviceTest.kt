package io.github.ifiokjr.canvist

import android.media.AudioFocusRequest
import android.media.AudioManager
import android.os.SystemClock
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.CopyOnWriteArrayList

/**
 * Dictation's system pieces on a real device: the microphone's permission
 * check, audio focus and device callbacks, and the messages people see. The
 * JVM tests cover sessions with fakes.
 */
@RunWith(AndroidJUnit4::class)
class DictationDeviceTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val context = instrumentation.targetContext

    @Test
    fun theMicrophoneNeedsRecordAudio() {
        // The test APK does not ask for RECORD_AUDIO.
        val error = runCatching { MicrophoneCapture(context).start { } }.exceptionOrNull()

        assertTrue("$error", error is DictationException.MicrophoneDenied)
    }

    @Test
    fun watchingAudioHoldsFocusQuietly() {
        val events = CopyOnWriteArrayList<CaptureEvent>()

        // Android grants audio focus to the app in the foreground, as an app
        // dictating from its own button is.
        ActivityScenario.launch(HostActivity::class.java).use { scenario ->
            var watch: AutoCloseable? = null
            scenario.onActivity { activity -> watch = SystemAudioEvents(activity).watch(events::add) }
            // Registering for device callbacks reports every device already
            // connected; none of them is a change.
            SystemClock.sleep(QUIET_MILLIS)
            scenario.onActivity { watch?.close() }
        }

        assertEquals(emptyList<CaptureEvent>(), events)
    }

    @Test
    fun losingAudioFocusInterrupts() {
        val events = CopyOnWriteArrayList<CaptureEvent>()
        val audio = context.getSystemService(AudioManager::class.java)
        // Another app recording, as a call would.
        val other =
            AudioFocusRequest
                .Builder(AudioManager.AUDIOFOCUS_GAIN_TRANSIENT_EXCLUSIVE)
                .setOnAudioFocusChangeListener { }
                .build()

        ActivityScenario.launch(HostActivity::class.java).use { scenario ->
            var watch: AutoCloseable? = null
            scenario.onActivity { activity -> watch = SystemAudioEvents(activity).watch(events::add) }
            SystemClock.sleep(QUIET_MILLIS)
            scenario.onActivity { audio.requestAudioFocus(other) }
            SystemClock.sleep(QUIET_MILLIS)
            scenario.onActivity {
                watch?.close()
                audio.abandonAudioFocusRequest(other)
            }
        }

        assertEquals(listOf(CaptureEvent.INTERRUPTED), events)
    }

    @Test
    fun failuresExplainThemselves() {
        val failures =
            listOf(
                DictationException.MicrophoneDenied(),
                DictationException.UnsupportedLanguage("fr-FR"),
                DictationException.RecognizerUnavailable("none"),
                DictationException.Audio("busy"),
                DictationException.Relay(DictationErrorCode.UNAUTHORIZED, "expired"),
                DictationException.Relay(DictationErrorCode.LIMIT, "too long"),
                DictationException.Connection("closed"),
            )
        val descriptions = failures.map { it.describe(context) }

        assertTrue(descriptions.all { it.isNotBlank() })
        assertNotEquals("an expired token says so", descriptions[6], descriptions[4])
        assertTrue("the message for people never carries the log details", descriptions.none { "expired" in it })
    }

    private companion object {
        const val QUIET_MILLIS = 300L
    }
}
