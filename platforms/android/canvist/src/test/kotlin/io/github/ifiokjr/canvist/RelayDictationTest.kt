package io.github.ifiokjr.canvist

import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Dictation through the relay, driven by scripted audio and a fake relay
 * that replays scripted messages into the real editor: no microphone or
 * network.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class RelayDictationTest {
    private val editor = dictationEditor()
    private val capture = FakeCapture()
    private val events = FakeAudioEvents()

    private fun source(relay: FakeRelay): RelayDictationSource {
        val connect = { relay }

        return RelayDictationSource(language = "en", polish = true, capture, events, connect)
    }

    private fun TestScope.dictation() = CanvistDictation(editor, this)

    private fun failure(dictation: CanvistDictation) = (dictation.state.value as DictationState.Failed).error

    @Test
    fun aSessionSendsStartAudioThenStopAndInsertsFinalText() =
        runTest {
            val relay = FakeRelay.scripted(start = listOf(READY, PARTIAL_WORLD), stop = listOf(FINAL_WORLD, DONE))
            val dictation = dictation()

            dictation.start(source(relay))
            assertEquals(DictationState.Connecting, dictation.state.value)
            runCurrent()
            assertEquals(DictationState.Listening, dictation.state.value)
            assertEquals(" world", editor.dictationDraft())
            assertEquals("drafts stay out of the document", "Hello", editor.plainText())

            dictation.stop()
            assertEquals(DictationState.Finishing, dictation.state.value)
            dictation.session?.join()

            assertEquals(DictationState.Idle, dictation.state.value)
            assertEquals("Hello world.", editor.plainText())
            assertNull(editor.dictationDraft())
            assertTrue(capture.stopped)
            assertTrue(relay.closed)
            assertFalse("the session stopped watching the system", events.watching)

            val start = relay.texts.first()
            assertTrue(start, start.contains("\"type\":\"start\""))
            assertTrue(start, start.contains("\"context\":\"Hello\""))
            assertTrue(start, start.contains("\"language\":\"en\""))
            assertEquals("""{"type":"stop"}""", relay.texts.last())
            assertTrue("the start message comes first", relay.sent.first() is Sent.Text)
            assertTrue("the stop message comes last", relay.sent.last() is Sent.Text)
            assertEquals("one binary frame per chunk", capture.chunks.size, relay.audio.size)
            assertEquals(capture.chunks.map { it.toList() }, relay.audio.map { it.bytes.toList() })
        }

    @Test
    fun aFatalRelayErrorFailsAndLeavesNoDraft() =
        runTest {
            val relay =
                FakeRelay.replying(
                    "start",
                    READY,
                    PARTIAL_WORLD,
                    """{"type":"error","code":"provider","message":"upstream closed","fatal":true}""",
                )
            val dictation = dictation()

            dictation.start(source(relay))
            dictation.session?.join()

            val error = failure(dictation) as DictationException.Relay
            assertEquals(DictationErrorCode.PROVIDER, error.code)
            assertEquals("upstream closed", error.relayMessage)
            assertNull(editor.dictationDraft())
            assertEquals("Hello", editor.plainText())
            assertTrue(capture.stopped)
            assertTrue(relay.closed)
        }

    @Test
    fun aNonFatalRelayErrorKeepsListening() =
        runTest {
            val polishFailed = """{"type":"error","code":"polish","message":"timed out","fatal":false}"""
            val rawFinal = """{"type":"final","segment":0,"text":"world"}"""
            val relay = FakeRelay.scripted(start = listOf(READY, polishFailed, rawFinal), stop = listOf(DONE))
            val dictation = dictation()

            dictation.start(source(relay))
            runCurrent()
            assertEquals("Hello world", editor.plainText())
            assertEquals(DictationState.Listening, dictation.state.value)

            dictation.stop()
            dictation.session?.join()
            assertEquals(DictationState.Idle, dictation.state.value)
        }

    @Test
    fun stoppingMidDraftClearsIt() =
        runTest {
            val relay = FakeRelay.scripted(start = listOf(READY, PARTIAL_WORLD), stop = listOf(DONE))
            val dictation = dictation()

            dictation.start(source(relay))
            runCurrent()
            assertEquals(" world", editor.dictationDraft())

            dictation.stop()
            dictation.session?.join()

            assertEquals(DictationState.Idle, dictation.state.value)
            assertNull(editor.dictationDraft())
            assertEquals("Hello", editor.plainText())
        }

    @Test
    fun stoppingWhileConnectingEndsTheSession() =
        runTest {
            val relay = FakeRelay.replying("stop", DONE)
            val dictation = dictation()

            dictation.start(source(relay))
            dictation.stop()
            dictation.session?.join()

            assertEquals(DictationState.Idle, dictation.state.value)
            assertTrue(capture.stopped)
            assertEquals("""{"type":"stop"}""", relay.texts.last())
        }

    @Test
    fun cancellingMidDraftClearsIt() =
        runTest {
            val relay = FakeRelay.replying("start", READY, PARTIAL_WORLD)
            val dictation = dictation()

            dictation.start(source(relay))
            runCurrent()
            assertEquals(" world", editor.dictationDraft())

            dictation.cancel()
            dictation.session?.join()

            assertEquals(DictationState.Idle, dictation.state.value)
            assertNull(editor.dictationDraft())
            assertTrue(capture.stopped)
            assertTrue(relay.closed)
        }

    @Test
    fun theScopeEndingClearsTheDraft() =
        runTest {
            val relay = FakeRelay.replying("start", READY, PARTIAL_WORLD)
            // A screen's scope, ending while the person is still talking.
            val screen = TestScope(testScheduler)
            val dictation = CanvistDictation(editor, screen)

            dictation.start(source(relay))
            runCurrent()
            assertEquals(" world", editor.dictationDraft())

            screen.cancel()
            runCurrent()

            assertEquals(DictationState.Idle, dictation.state.value)
            assertNull(editor.dictationDraft())
            assertTrue(capture.stopped)
            assertTrue(relay.closed)
        }

    @Test
    fun theSocketClosingMidDraftFailsAndClearsIt() =
        runTest {
            val relay = FakeRelay.replying("start", READY, PARTIAL_WORLD)
            val dictation = dictation()

            dictation.start(source(relay))
            runCurrent()
            relay.drop()
            dictation.session?.join()

            assertTrue(failure(dictation) is DictationException.Connection)
            assertNull(editor.dictationDraft())
        }

    @Test
    fun anUnreadableMessageFailsTheSession() =
        runTest {
            val relay = FakeRelay.replying("start", READY, PARTIAL_WORLD, "not json")
            val dictation = dictation()

            dictation.start(source(relay))
            dictation.session?.join()

            assertTrue(failure(dictation) is DictationException.Connection)
            assertNull(editor.dictationDraft())
        }

    @Test
    fun aRefusedMicrophoneFailsWithoutConnecting() =
        runTest {
            capture.failure = DictationException.MicrophoneDenied()
            var connected = false
            val source =
                RelayDictationSource(language = null, polish = true, capture, events) {
                    connected = true
                    FakeRelay.replying("start")
                }
            val dictation = dictation()

            dictation.start(source)
            dictation.session?.join()

            assertTrue(failure(dictation) is DictationException.MicrophoneDenied)
            assertFalse(connected)
        }

    @Test
    fun startingWhileDictatingDoesNothing() =
        runTest {
            val first = FakeRelay.replying("start", READY)
            val second = FakeRelay.replying("start", READY)
            val dictation = dictation()

            dictation.start(source(first))
            dictation.start(source(second))
            runCurrent()

            assertTrue(first.texts.isNotEmpty())
            assertTrue(second.texts.isEmpty())
            dictation.cancel()
        }

    @Test
    fun stateIsAFlowAButtonCanCollect() =
        runTest {
            val relay = FakeRelay.scripted(start = listOf(READY), stop = listOf(DONE))
            val dictation = dictation()
            val seen = mutableListOf<DictationState>()
            val collector = launch { dictation.state.collect(seen::add) }
            runCurrent()

            dictation.start(source(relay))
            runCurrent()
            dictation.stop()
            dictation.session?.join()
            runCurrent()
            collector.cancel()

            assertEquals(
                listOf(
                    DictationState.Idle,
                    DictationState.Connecting,
                    DictationState.Listening,
                    DictationState.Finishing,
                    DictationState.Idle,
                ),
                seen,
            )
        }

    @Test
    fun convertsSamplesToLittleEndianBytes() {
        val ignored: Short = 0x7FFF
        val bytes = Pcm.littleEndian(shortArrayOf(1, -2, 0x1234, ignored), count = 3)

        assertEquals(listOf<Byte>(0x01, 0x00, 0xFE.toByte(), 0xFF.toByte(), 0x34, 0x12), bytes.toList())
        assertEquals("100 ms at 16 kHz", Pcm.SAMPLE_RATE / 10, Pcm.CHUNK_SAMPLES)
    }
}
