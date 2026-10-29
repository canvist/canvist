package io.github.ifiokjr.canvist.sample

import android.content.Context
import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import io.github.ifiokjr.canvist.CanvistException
import io.github.ifiokjr.canvist.CanvistView
import io.github.ifiokjr.canvist.Command
import io.github.ifiokjr.canvist.DictationSource
import io.github.ifiokjr.canvist.DocFormat
import io.github.ifiokjr.canvist.Editor
import io.github.ifiokjr.canvist.EditorChange
import io.github.ifiokjr.canvist.OnDeviceDictationSource
import io.github.ifiokjr.canvist.RelayDictationSource
import io.github.ifiokjr.canvist.StarterKit
import io.github.ifiokjr.canvist.query

/** One editor and the state its toolbar shows. */
class EditorModel(
    context: Context,
    markdown: String,
    val label: String,
) {
    val kit = StarterKit()
    val view =
        CanvistView(context, kit.extensions).apply {
            contentDescription = label
            tag = "${label.lowercase()}-editor"
            editor.load(DocFormat.MARKDOWN, markdown)
        }

    var active by mutableStateOf(emptySet<Format>())
        private set
    var canUndo by mutableStateOf(false)
        private set
    var canRedo by mutableStateOf(false)
        private set

    /** Called after every change, for collaboration. */
    var changed: ((EditorChange) -> Unit)? = null

    init {
        view.onChange = { change ->
            refresh()
            changed?.invoke(change)
        }
        refresh()
    }

    fun toggle(format: Format) = perform { view.editor.run(format.command(kit)) }

    fun undo() = perform { view.editor.undo() }

    fun redo() = perform { view.editor.redo() }

    private fun perform(body: () -> Unit) {
        try {
            body()
        } catch (error: CanvistException) {
            Log.e(TAG, "command failed", error)
        }
    }

    /** Read the toolbar state back from the editor. */
    private fun refresh() {
        val editor = view.editor
        active = Format.entries.filter { it.isActive(kit, editor) }.toSet()
        canUndo = editor.canUndo()
        canRedo = editor.canRedo()
    }

    private companion object {
        const val TAG = "CanvistSample"
    }
}

/**
 * Where the sample's dictation comes from: a relay when the app is built with
 * one (see `build.gradle.kts`), otherwise the device's own recognizer when it
 * has one.
 */
sealed interface DictationSetup {
    data class Relay(
        val url: String,
        val token: String,
    ) : DictationSetup

    data object OnDevice : DictationSetup

    data class Unavailable(
        val reason: String,
    ) : DictationSetup

    /** A new source for a session, or `null` when dictation is unavailable. */
    fun source(context: Context): DictationSource? =
        when (this) {
            is Relay -> RelayDictationSource(context, url, token)
            OnDevice -> OnDeviceDictationSource(context)
            is Unavailable -> null
        }

    companion object {
        /** The setup this build and device allow. */
        fun current(context: Context): DictationSetup =
            when {
                BuildConfig.DICTATION_URL.isNotEmpty() && BuildConfig.DICTATION_TOKEN.isNotEmpty() -> {
                    Relay(BuildConfig.DICTATION_URL, BuildConfig.DICTATION_TOKEN)
                }

                OnDeviceDictationSource.isAvailable(context) -> {
                    OnDevice
                }

                else -> {
                    Unavailable("Dictation needs a relay or a speech recognizer on this device.")
                }
            }
    }
}

/** The toolbar's formats: a typed command, and the query that says whether it applies to the selection. */
enum class Format(
    val label: String,
    val title: String,
) {
    BOLD("B", "Bold"),
    ITALIC("I", "Italic"),
    UNDERLINE("U", "Underline"),
    STRIKE("S", "Strikethrough"),
    HEADING("H", "Heading"),
    BULLET_LIST("•", "Bulleted list"),
    TASK_LIST("☑", "Checklist"),
    QUOTE("❝", "Quote"),
    CODE("{ }", "Code block"),
    ;

    fun command(kit: StarterKit): Command =
        when (this) {
            BOLD -> kit.bold.toggleBold()
            ITALIC -> kit.italic.toggleItalic()
            UNDERLINE -> kit.underline.toggleUnderline()
            STRIKE -> kit.strike.toggleStrike()
            HEADING -> kit.heading.toggleHeading(level = 2)
            BULLET_LIST -> kit.bulletList.toggleBulletList()
            TASK_LIST -> kit.taskList.toggleTaskList()
            QUOTE -> kit.blockquote.toggleBlockquote()
            CODE -> kit.codeBlock.toggleCodeBlock()
        }

    fun isActive(
        kit: StarterKit,
        editor: Editor,
    ): Boolean =
        when (this) {
            BOLD -> editor.query(kit.bold.isBold())
            ITALIC -> editor.query(kit.italic.isItalic())
            UNDERLINE -> editor.query(kit.underline.isUnderline())
            STRIKE -> editor.query(kit.strike.isStrike())
            HEADING -> editor.query(kit.heading.headingLevel()) != null
            BULLET_LIST -> editor.query(kit.bulletList.isBulletList())
            TASK_LIST -> editor.query(kit.taskList.isTaskList())
            QUOTE -> editor.query(kit.blockquote.isBlockquote())
            CODE -> editor.query(kit.codeBlock.isCodeBlock())
        }
}

/** Two editors joined over Yjs in memory, as two devices would be over a network. */
class CollaborationModel(
    context: Context,
) {
    val alice = EditorModel(context, "# Shared notes\n\nType in either editor.", "Alice")
    val bob = EditorModel(context, "", "Bob")

    init {
        alice.view.editor.share(1uL)
        bob.view.editor.join(alice.view.editor.encodeState(), 2uL)
        alice.changed = { change -> send(change, alice.view.editor, bob.view.editor) }
        bob.changed = { change -> send(change, bob.view.editor, alice.view.editor) }
    }

    /** Deliver [from]'s new Yjs updates to [to]. */
    private fun send(
        change: EditorChange,
        from: Editor,
        to: Editor,
    ) {
        if (!change.updates) return

        for (update in from.takeUpdates()) to.applyUpdate(update)
    }
}
