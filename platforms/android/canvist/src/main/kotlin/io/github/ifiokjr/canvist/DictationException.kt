package io.github.ifiokjr.canvist

import android.content.Context
import androidx.annotation.StringRes

/**
 * Why dictation failed.
 *
 * [message] is for logs: it never holds audio, transcripts, or tokens.
 * [describe] gives a sentence to show people using the app.
 */
sealed class DictationException(
    message: String,
    cause: Throwable? = null,
) : Exception(message, cause) {
    /** The person has not let the app record audio (`RECORD_AUDIO`). */
    class MicrophoneDenied : DictationException("microphone denied")

    /** The device's recognizer does not support [language], a BCP 47 tag. */
    class UnsupportedLanguage(
        val language: String,
    ) : DictationException("unsupported language $language")

    /** The device has no speech recognizer this app can use. */
    class RecognizerUnavailable(
        reason: String,
    ) : DictationException("no recognizer: $reason")

    /** The microphone or the recognizer could not capture audio. */
    class Audio(
        reason: String,
        cause: Throwable? = null,
    ) : DictationException("audio: $reason", cause)

    /** The relay ended the session with an error. [relayMessage] is for logs. */
    class Relay(
        val code: DictationErrorCode,
        val relayMessage: String,
    ) : DictationException("relay $code: $relayMessage")

    /** The connection to the relay failed or closed before the session finished. */
    class Connection(
        reason: String,
        cause: Throwable? = null,
    ) : DictationException("connection: $reason", cause)

    /** A sentence explaining the failure to people using the app, in [context]'s language. */
    fun describe(context: Context): String = context.getString(descriptionResource)

    @get:StringRes
    private val descriptionResource: Int
        get() =
            when (this) {
                is MicrophoneDenied -> R.string.canvist_dictation_microphone_denied
                is UnsupportedLanguage -> R.string.canvist_dictation_unsupported_language
                is RecognizerUnavailable -> R.string.canvist_dictation_recognizer_unavailable
                is Audio -> R.string.canvist_dictation_audio
                is Relay -> relayDescription(code)
                is Connection -> R.string.canvist_dictation_connection
            }

    private companion object {
        @StringRes
        fun relayDescription(code: DictationErrorCode): Int =
            when (code) {
                DictationErrorCode.UNAUTHORIZED -> R.string.canvist_dictation_unauthorized
                DictationErrorCode.LIMIT -> R.string.canvist_dictation_limit
                else -> R.string.canvist_dictation_connection
            }
    }
}
