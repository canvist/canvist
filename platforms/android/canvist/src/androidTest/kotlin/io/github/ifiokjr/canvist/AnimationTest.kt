package io.github.ifiokjr.canvist

import android.content.Context
import android.graphics.Canvas
import android.os.SystemClock
import android.view.View
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.atomic.AtomicInteger

/**
 * Animated text in a real window: the view repaints on every display refresh
 * while a dictation draft breathes or final text settles, and stops as soon
 * as nothing moves, when it leaves the window, or for reduced motion.
 */
@RunWith(AndroidJUnit4::class)
class AnimationTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()

    /** A view counting the frames it draws. */
    private class CountingView(
        context: Context,
    ) : CanvistView(context, StarterKit().extensions) {
        val draws = AtomicInteger()

        override fun onDraw(canvas: Canvas) {
            super.onDraw(canvas)
            draws.incrementAndGet()
        }
    }

    /** Run [body] with a view showing "Hello" in a window, the caret at the end and animations on. */
    private fun withWindow(body: (ActivityScenario<HostActivity>, CountingView) -> Unit) {
        ActivityScenario.launch(HostActivity::class.java).use { scenario ->
            var shown: CountingView? = null
            scenario.onActivity { activity ->
                val view = CountingView(activity)
                view.editor.load(DocFormat.MARKDOWN, "Hello")
                val end = view.editor.textLength()
                view.editor.select(end, end)
                activity.setContentView(view)
                shown = view
            }
            val view = shown!!

            // The view reads the system's animation scale when its window
            // gains focus; set the preference only after that.
            waitFor("window focus") { view.hasWindowFocus() }
            onMain { view.applyMotionPreference(reduced = false) }
            body(scenario, view)
        }
    }

    private fun onMain(body: () -> Unit) = instrumentation.runOnMainSync(body)

    private fun waitFor(
        description: String,
        condition: () -> Boolean,
    ) {
        val deadline = SystemClock.uptimeMillis() + TIMEOUT_MILLIS

        while (!condition()) {
            check(SystemClock.uptimeMillis() < deadline) { "timed out waiting for $description" }
            SystemClock.sleep(POLL_MILLIS)
        }
    }

    /** How many frames [view] draws in [WINDOW_MILLIS]. */
    private fun framesDrawn(view: CountingView): Int {
        val before = view.draws.get()
        SystemClock.sleep(WINDOW_MILLIS)

        return view.draws.get() - before
    }

    @Test
    fun framesFollowTheDisplayOnlyWhileTextAnimates() =
        withWindow { _, view ->
            // Let the first frames and the caret's first blink settle.
            SystemClock.sleep(WINDOW_MILLIS)
            assertFalse("an idle editor asks for no frames", view.animating)
            assertTrue("idle: at most the caret's blink", framesDrawn(view) <= IDLE_FRAMES)

            onMain { view.editor.dictate(0u, "world", false) }
            waitFor("the draft to animate") { view.animating }
            val breathing = framesDrawn(view)
            assertTrue("the draft breathes on every refresh: $breathing frames", breathing >= ANIMATED_FRAMES)

            onMain { view.editor.stopDictation() }
            waitFor("the frames to stop") { !view.animating }
            assertTrue("nothing is animating", framesDrawn(view) <= IDLE_FRAMES)
        }

    @Test
    fun finalTextSettlesThenStops() =
        withWindow { _, view ->
            onMain { view.editor.dictate(0u, "world.", true) }
            waitFor("the final text to settle") { view.animating }
            assertEquals("Hello world.", view.editor.plainText())

            // Settling lasts 450 ms.
            waitFor("the settled text to stop") { !view.animating }
            assertTrue(framesDrawn(view) <= IDLE_FRAMES)
        }

    @Test
    fun leavingTheWindowStopsTheFrames() =
        withWindow { scenario, view ->
            onMain { view.editor.dictate(0u, "world", false) }
            waitFor("the draft to animate") { view.animating }

            scenario.onActivity { activity -> activity.setContentView(View(activity)) }

            assertFalse("off screen", view.animating)
            assertEquals(0, framesDrawn(view))
        }

    @Test
    fun reducedMotionKeepsTheDraftStill() =
        withWindow { _, view ->
            onMain {
                view.applyMotionPreference(reduced = true)
                view.editor.dictate(0u, "world", false)
            }
            SystemClock.sleep(WINDOW_MILLIS)

            assertFalse(view.animating)
            assertFalse(view.editor.frame().animated)
            assertEquals("the draft still shows, steadily", " world", view.editor.dictationDraft())
            assertTrue(framesDrawn(view) <= IDLE_FRAMES)
        }

    private companion object {
        const val TIMEOUT_MILLIS = 5_000L
        const val POLL_MILLIS = 10L
        const val WINDOW_MILLIS = 500L

        /** The caret blinks every 500 ms, so a still view may draw once or twice in a window. */
        const val IDLE_FRAMES = 2

        /** A 60 Hz display refreshes 30 times in a window; an emulator under load manages fewer. */
        const val ANIMATED_FRAMES = 10
    }
}
