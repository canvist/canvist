package io.github.ifiokjr.canvist

import android.media.AudioManager
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Dictation through the system's audio changes: a call taking audio focus,
 * and headsets connecting. The tests send the system's events themselves.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class AudioFocusDictationTest {
    private val editor = dictationEditor()
    private val capture = FakeCapture()
    private val events = FakeAudioEvents()

    /** A relay that drafts "world" once started, and makes it final when stopped. */
    private val relay =
        FakeRelay.scripted(start = listOf(READY, PARTIAL_WORLD), stop = listOf(FINAL_WORLD, DONE))

    /** Start a session and let its draft show. */
    private fun TestScope.startDrafting(): CanvistDictation {
        val dictation = CanvistDictation(editor, this)
        dictation.start(RelayDictationSource(language = null, polish = true, capture, events) { relay })
        runCurrent()
        assertEquals(" world", editor.dictationDraft())

        return dictation
    }

    /** The session ended with the draft made final, and nothing provisional left. */
    private suspend fun assertFinishedWithFinalText(dictation: CanvistDictation) {
        dictation.session?.join()

        assertEquals(DictationState.Idle, dictation.state.value)
        assertEquals("Hello world.", editor.plainText())
        assertNull(editor.dictationDraft())
        assertTrue(capture.stopped)
    }

    @Test
    fun losingAudioFocusFinishesWithTheWordsSaid() =
        runTest {
            val dictation = startDrafting()

            events.emit(CaptureEvent.INTERRUPTED)
            assertEquals(DictationState.Finishing, dictation.state.value)

            assertFinishedWithFinalText(dictation)
            assertEquals("""{"type":"stop"}""", relay.texts.last())
        }

    @Test
    fun aNewMicrophoneRestartsCaptureAndKeepsListening() =
        runTest {
            val dictation = startDrafting()

            events.emit(CaptureEvent.INPUT_CHANGED)
            runCurrent()

            assertEquals(1, capture.restarts)
            assertEquals(DictationState.Listening, dictation.state.value)
            val captured = capture.chunks.size + capture.rerouted.size
            assertEquals("the new route's audio is sent too", captured, relay.audio.size)

            dictation.stop()
            assertFinishedWithFinalText(dictation)
        }

    @Test
    fun aMicrophoneThatCannotRestartFinishes() =
        runTest {
            capture.restartFailure = DictationException.Audio("no microphone left")
            val dictation = startDrafting()

            events.emit(CaptureEvent.INPUT_CHANGED)
            assertEquals(DictationState.Finishing, dictation.state.value)

            assertFinishedWithFinalText(dictation)
        }

    @Test
    fun eventsAfterTheSessionDoNothing() =
        runTest {
            val dictation = startDrafting()
            dictation.stop()
            dictation.session?.join()
            assertFalse(events.watching)

            events.emit(CaptureEvent.INPUT_CHANGED)
            events.emit(CaptureEvent.INTERRUPTED)

            assertEquals(0, capture.restarts)
            assertEquals(DictationState.Idle, dictation.state.value)
        }

    @Test
    fun onlyLosingFocusInterrupts() {
        assertTrue(CaptureSupervisor.interrupts(AudioManager.AUDIOFOCUS_LOSS))
        assertTrue("a phone call", CaptureSupervisor.interrupts(AudioManager.AUDIOFOCUS_LOSS_TRANSIENT))
        val duck = AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK
        assertFalse("ducking is for playback", CaptureSupervisor.interrupts(duck))
        assertFalse(CaptureSupervisor.interrupts(AudioManager.AUDIOFOCUS_GAIN))
    }
}
