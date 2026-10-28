package io.github.ifiokjr.canvist

import android.speech.SpeechRecognizer
import io.github.ifiokjr.canvist.RecognitionEvent.Failed
import io.github.ifiokjr.canvist.RecognitionEvent.Final
import io.github.ifiokjr.canvist.RecognitionEvent.Partial
import io.github.ifiokjr.canvist.RecognitionEvent.Ready
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Dictation with the device's recognizer, replaying scripted recognizer events into the real editor. */
@OptIn(ExperimentalCoroutinesApi::class)
class OnDeviceDictationTest {
    private val editor = dictationEditor("Hi")
    private val events = FakeAudioEvents()

    private fun failure(code: Int) = OnDeviceDictationSource.failure(code, "en")

    private fun TestScope.start(recognizer: FakeRecognizer): CanvistDictation {
        val dictation = CanvistDictation(editor, this)
        dictation.start(OnDeviceDictationSource(language = "en-US", recognizer, events))
        runCurrent()

        return dictation
    }

    @Test
    fun resultsBecomeSegments() {
        val source = OnDeviceDictationSource(language = null, FakeRecognizer(mutableListOf()), events)

        source.dictate(" hello", isFinal = false, editor)
        assertEquals(" hello", editor.dictationDraft())

        source.dictate(" Hello.", isFinal = true, editor)
        source.dictate(" again", isFinal = false, editor)

        assertEquals("Hi Hello.", editor.plainText())
        assertEquals("the next segment's draft", " again", editor.dictationDraft())
    }

    @Test
    fun listensAgainAfterEachUtteranceUntilStopped() =
        runTest {
            val recognizer =
                FakeRecognizer(
                    utterances =
                        mutableListOf(
                            listOf(Ready, Partial("hello"), Final("Hello.")),
                            // A pause with nothing said.
                            listOf(Failed(SpeechRecognizer.ERROR_SPEECH_TIMEOUT)),
                            listOf(Partial("again")),
                        ),
                    whenStopped = listOf(Final("again")),
                )
            val dictation = start(recognizer)

            assertEquals(DictationState.Listening, dictation.state.value)
            assertEquals("Hi Hello.", editor.plainText())
            assertEquals(" again", editor.dictationDraft())
            assertEquals(3, recognizer.listens)

            dictation.stop()
            dictation.session?.join()

            assertEquals(DictationState.Idle, dictation.state.value)
            assertEquals("Hi Hello. again", editor.plainText())
            assertNull(editor.dictationDraft())
            assertEquals(1, recognizer.stops)
            assertTrue(recognizer.closed)
        }

    @Test
    fun aRecognizerErrorFailsAndLeavesNoDraft() =
        runTest {
            val recognizer =
                FakeRecognizer(
                    mutableListOf(
                        listOf(
                            Ready,
                            Partial("hello"),
                            Failed(SpeechRecognizer.ERROR_AUDIO),
                        ),
                    ),
                )
            val dictation = start(recognizer)
            dictation.session?.join()

            assertTrue((dictation.state.value as DictationState.Failed).error is DictationException.Audio)
            assertNull(editor.dictationDraft())
            assertEquals("Hi", editor.plainText())
            assertTrue(recognizer.closed)
        }

    @Test
    fun aRefusedMicrophoneFails() =
        runTest {
            val recognizer = FakeRecognizer(mutableListOf())
            recognizer.failure = DictationException.MicrophoneDenied()
            val dictation = start(recognizer)
            dictation.session?.join()

            assertTrue((dictation.state.value as DictationState.Failed).error is DictationException.MicrophoneDenied)
        }

    @Test
    fun aNewMicrophoneEndsTheUtteranceAndListensAgain() =
        runTest {
            val recognizer =
                FakeRecognizer(
                    utterances = mutableListOf(listOf(Ready, Partial("hello"))),
                    whenStopped = listOf(Final("hello")),
                )
            val dictation = start(recognizer)

            events.emit(CaptureEvent.INPUT_CHANGED)
            runCurrent()

            assertEquals("Hi hello", editor.plainText())
            assertEquals("listening on the new route", 2, recognizer.listens)
            assertEquals(DictationState.Listening, dictation.state.value)
            dictation.cancel()
        }

    @Test
    fun losingAudioFocusFinishesWithTheWordsSaid() =
        runTest {
            val recognizer =
                FakeRecognizer(
                    utterances = mutableListOf(listOf(Ready, Partial("hello"))),
                    whenStopped = listOf(Final("hello")),
                )
            val dictation = start(recognizer)

            events.emit(CaptureEvent.INTERRUPTED)
            assertEquals(DictationState.Finishing, dictation.state.value)
            dictation.session?.join()

            assertEquals(DictationState.Idle, dictation.state.value)
            assertEquals("Hi hello", editor.plainText())
        }

    @Test
    fun aRecognizerSilentAfterStoppingStillEnds() =
        runTest {
            val recognizer = FakeRecognizer(mutableListOf(listOf(Ready, Partial("hello"))))
            val dictation = start(recognizer)

            dictation.stop()
            advanceTimeBy(PAST_THE_FINISH_TIMEOUT_MILLIS)
            runCurrent()

            assertEquals(DictationState.Idle, dictation.state.value)
            assertNull("the unfinished words go", editor.dictationDraft())
            assertEquals("Hi", editor.plainText())
        }

    @Test
    fun anUtteranceWithNoWordsClearsItsDraft() =
        runTest {
            val recognizer =
                FakeRecognizer(
                    mutableListOf(
                        listOf(Ready, Partial("um"), Final("")),
                        listOf(Partial("hello")),
                    ),
                )
            val dictation = start(recognizer)

            assertEquals("Hi", editor.plainText())
            assertEquals("the same segment drafts again", " hello", editor.dictationDraft())
            dictation.cancel()
        }

    @Test
    fun recognizerErrorsMapToFailures() {
        assertNull(failure(SpeechRecognizer.ERROR_NO_MATCH))
        assertNull(failure(SpeechRecognizer.ERROR_SPEECH_TIMEOUT))
        assertTrue(
            failure(SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS) is DictationException.MicrophoneDenied,
        )
        assertTrue(failure(SpeechRecognizer.ERROR_NETWORK) is DictationException.Connection)
        assertTrue(failure(SpeechRecognizer.ERROR_AUDIO) is DictationException.Audio)
        assertTrue(
            failure(SpeechRecognizer.ERROR_RECOGNIZER_BUSY) is DictationException.RecognizerUnavailable,
        )

        val unsupported = OnDeviceDictationSource.failure(LANGUAGE_NOT_SUPPORTED, "fr-FR")
        val language = unsupported as DictationException.UnsupportedLanguage
        assertEquals("fr-FR", language.language)
    }

    private companion object {
        /** `SpeechRecognizer.ERROR_LANGUAGE_NOT_SUPPORTED`, from Android 12. */
        const val LANGUAGE_NOT_SUPPORTED = 12

        /** Longer than the source waits for a stopped recognizer to answer. */
        const val PAST_THE_FINISH_TIMEOUT_MILLIS = 5_000L
    }
}
