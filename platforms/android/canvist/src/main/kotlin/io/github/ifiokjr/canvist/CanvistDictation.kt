package io.github.ifiokjr.canvist

import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/**
 * Dictation into a [CanvistView]: speech becomes text at the caret as
 * someone talks.
 *
 * The words still being said show faint at the caret and rewrite themselves
 * as the speech model changes its mind; final words settle into the
 * document, each segment as one undo step. Drafts never enter the document,
 * so collaborators, saves, and undo only ever see final text.
 *
 * ```kotlin
 * val dictation = CanvistDictation(view, lifecycleScope)
 * dictation.start(RelayDictationSource(context, relayUrl, token))
 * // Later, when the person taps the button again:
 * dictation.stop()
 * ```
 *
 * Sessions run in [scope], so they end with it: pass a lifecycle's scope
 * (`lifecycleScope`, `viewModelScope`, or Compose's `rememberCoroutineScope`)
 * and a session never outlives the screen. However a session ends (done,
 * failed, cancelled, or its scope gone) its draft goes with it. Call every
 * method on the main thread.
 *
 * An editor without a view, such as one React Native or Lynx hosts, dictates
 * the same way; the view showing it repaints from its change notifications.
 */
class CanvistDictation(
    private val editor: Editor,
    private val scope: CoroutineScope,
) {
    /** Dictation into [view]'s editor, with sessions running in [scope]. */
    constructor(view: CanvistView, scope: CoroutineScope) : this(view.editor, scope)

    private val mutableState = MutableStateFlow<DictationState>(DictationState.Idle)

    /** Where the current or last session is. */
    val state: StateFlow<DictationState> = mutableState.asStateFlow()

    private var source: DictationSource? = null

    /** The running session, for tests to join. */
    internal var session: Job? = null
        private set

    /** Start dictating at the caret from [source]. Does nothing while a session is running. */
    fun start(source: DictationSource) {
        if (state.value.isActive) return

        this.source = source
        mutableState.value = DictationState.Connecting
        session = scope.launch { run(source) }
    }

    /** Stop listening. Words already said still become final; the session then ends. */
    fun stop() {
        if (!state.value.isListening) return

        mutableState.value = DictationState.Finishing
        source?.finish()
    }

    /** End the session now, dropping the words that are not final yet. */
    fun cancel() {
        session?.cancel()
    }

    private suspend fun run(source: DictationSource) {
        // Cancellation, the scope ending included, is not a failure.
        var ending: DictationState = DictationState.Idle

        try {
            source.run(editor, ::advance)
        } catch (error: DictationException) {
            Log.e(TAG, "dictation failed: ${error.message}")
            ending = DictationState.Failed(error)
        } finally {
            end(ending)
        }
    }

    /**
     * Follow the source: listening once speech is being transcribed, and
     * finishing when it stops listening on its own, such as for a phone call.
     */
    private fun advance(progress: DictationProgress) {
        val current = state.value
        val next =
            when (progress) {
                DictationProgress.LISTENING -> DictationState.Listening.takeIf { current == DictationState.Connecting }
                DictationProgress.FINISHING -> DictationState.Finishing.takeIf { current.isListening }
            }

        mutableState.value = next ?: current
    }

    /** The session is over: whatever was still a draft goes, so nothing provisional outlives it. */
    private fun end(state: DictationState) {
        editor.stopDictation()
        source = null
        session = null
        mutableState.value = state
    }

    private companion object {
        const val TAG = "CanvistDictation"
    }
}

/** Where a dictation session is. */
sealed interface DictationState {
    /** Not dictating. */
    data object Idle : DictationState

    /** Reaching the microphone and the speech model. */
    data object Connecting : DictationState

    /** Speech is being transcribed. */
    data object Listening : DictationState

    /** The microphone is off, and the last words are becoming final. */
    data object Finishing : DictationState

    /** The last session failed with [error]. Starting again clears it. */
    data class Failed(
        val error: DictationException,
    ) : DictationState

    /** Whether a session is running. */
    val isActive: Boolean get() = this == Connecting || this == Listening || this == Finishing

    /** Whether the microphone may still be on: connecting or listening. */
    val isListening: Boolean get() = this == Connecting || this == Listening
}

/**
 * Where dictated speech comes from: canvist's relay ([RelayDictationSource]),
 * or the device's own recognizer ([OnDeviceDictationSource]). A source runs
 * one session at a time, on the main thread.
 */
interface DictationSource {
    /**
     * Listen, and dictate what is said into [editor] until [finish] or
     * cancellation. Reports [progress] as the session goes on, returns when
     * it is over, and throws a [DictationException] when it fails.
     * [CanvistDictation] drops any draft left once it returns.
     */
    suspend fun run(
        editor: Editor,
        progress: (DictationProgress) -> Unit,
    )

    /** Stop listening, let the words already said become final, then return from [run]. */
    fun finish()
}

/** How a dictation session is going, as its source reports it. */
enum class DictationProgress {
    /** Speech is being transcribed. */
    LISTENING,

    /**
     * The source stopped listening on its own, as when a phone call takes
     * audio focus: the words already said are becoming final.
     */
    FINISHING,
}
