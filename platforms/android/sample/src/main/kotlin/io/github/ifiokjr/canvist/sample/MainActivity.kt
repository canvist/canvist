package io.github.ifiokjr.canvist.sample

import android.Manifest
import android.content.pm.PackageManager
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import io.github.ifiokjr.canvist.CanvistDictation
import io.github.ifiokjr.canvist.DictationState

/** The sample: a rich-text editor with a formatting toolbar, and two editors collaborating. */
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val markdown = if (intent.getBooleanExtra(EXTRA_EMPTY_DOCUMENT, false)) "" else WELCOME
        setContent {
            val colors = if (isSystemInDarkTheme()) darkColorScheme() else lightColorScheme()
            MaterialTheme(colorScheme = colors) { SampleApp(markdown) }
        }
    }

    companion object {
        /** UI tests start from an empty document. */
        const val EXTRA_EMPTY_DOCUMENT = "emptyDocument"
    }
}

@Composable
fun SampleApp(markdown: String) {
    val context = LocalContext.current
    val editor = remember { EditorModel(context, markdown, "Editor") }
    // Dictation ends with the app's composition, and its draft with it.
    val scope = rememberCoroutineScope()
    val dictation = remember { CanvistDictation(editor.view, scope) }
    val setup = remember { DictationSetup.current(context) }
    val collaboration = remember { CollaborationModel(context) }
    var tab by remember { mutableIntStateOf(0) }

    Scaffold(
        modifier = Modifier.imePadding(),
        bottomBar = {
            NavigationBar {
                TABS.forEachIndexed { index, (title, icon) ->
                    NavigationBarItem(
                        selected = tab == index,
                        onClick = { tab = index },
                        icon = { Text(icon) },
                        label = { Text(title) },
                    )
                }
            }
        },
    ) { padding ->
        when (tab) {
            0 -> EditorScreen(editor, dictation, setup, padding)
            else -> CollaborationScreen(collaboration, padding)
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun EditorScreen(
    model: EditorModel,
    dictation: CanvistDictation,
    setup: DictationSetup,
    padding: PaddingValues,
) {
    val state by dictation.state.collectAsState()
    val note = dictationNote(state, setup)

    Column(Modifier.fillMaxSize().padding(padding)) {
        TopAppBar(
            title = { Text("Canvist") },
            actions = {
                DictationButton(state, setup, rememberDictationToggle(dictation, setup))
                TextButton(onClick = model::undo, enabled = model.canUndo) { Text("Undo") }
                TextButton(onClick = model::redo, enabled = model.canRedo) { Text("Redo") }
            },
        )
        FormatBar(model)

        if (note != null) {
            Text(
                note,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 20.dp, vertical = 8.dp)
                        .testTag("dictation-note"),
            )
        }

        HorizontalDivider()
        AndroidView(factory = { model.view }, modifier = Modifier.fillMaxSize())
    }
}

/**
 * Starts and stops dictation. It turns red while a session runs and pulses
 * while listening, and is disabled when the app has no way to dictate.
 */
@Composable
fun DictationButton(
    state: DictationState,
    setup: DictationSetup,
    onClick: () -> Unit,
) {
    val container = if (state.isActive) MaterialTheme.colorScheme.errorContainer else Color.Transparent

    TextButton(
        onClick = onClick,
        enabled = setup !is DictationSetup.Unavailable && state != DictationState.Finishing,
        modifier =
            Modifier
                .background(container, RoundedCornerShape(8.dp))
                .testTag("dictate")
                .semantics { contentDescription = if (state.isActive) "Stop dictating" else "Dictate" },
    ) {
        if (state == DictationState.Listening) ListeningMic() else Text(MIC)
    }
}

/** The mic, breathing while it listens. */
@Composable
private fun ListeningMic() {
    val transition = rememberInfiniteTransition(label = "listening")
    val alpha by transition.animateFloat(
        initialValue = 1f,
        targetValue = 0.35f,
        animationSpec = infiniteRepeatable(tween(durationMillis = BREATH_MILLIS), RepeatMode.Reverse),
        label = "listening alpha",
    )

    Text(MIC, modifier = Modifier.alpha(alpha))
}

/**
 * What the dictation button does: stop a running session, or start one,
 * asking for the microphone first. A refused microphone still starts the
 * session, which fails at once and says why under the toolbar.
 */
@Composable
private fun rememberDictationToggle(
    dictation: CanvistDictation,
    setup: DictationSetup,
): () -> Unit {
    val context = LocalContext.current
    val start = { setup.source(context)?.let(dictation::start) ?: Unit }
    val permission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { start() }

    return {
        val granted = context.checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED

        when {
            dictation.state.value.isActive -> dictation.stop()
            granted -> start()
            else -> permission.launch(Manifest.permission.RECORD_AUDIO)
        }
    }
}

/** Why dictation is off or failed, shown under the toolbar so people see it without tapping. */
@Composable
private fun dictationNote(
    state: DictationState,
    setup: DictationSetup,
): String? {
    val context = LocalContext.current

    if (state is DictationState.Failed) return state.error.describe(context)

    return (setup as? DictationSetup.Unavailable)?.reason
}

/** Toggles for the toolbar's formats, lit while the selection has them. */
@Composable
fun FormatBar(model: EditorModel) {
    Row(
        modifier = Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(2.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        for (format in Format.entries) {
            val active = format in model.active
            val highlight = if (active) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surface
            val weight = if (format == Format.BOLD) FontWeight.Bold else FontWeight.Normal
            TextButton(
                onClick = { model.toggle(format) },
                modifier =
                    Modifier
                        .size(width = 44.dp, height = 40.dp)
                        .background(highlight, RoundedCornerShape(8.dp))
                        .semantics {
                            contentDescription = format.title
                            selected = active
                        },
                contentPadding = PaddingValues(0.dp),
            ) {
                Text(format.label, fontWeight = weight)
            }
        }
    }
}

/** Two editors joined over Yjs. */
@Composable
fun CollaborationScreen(
    model: CollaborationModel,
    padding: PaddingValues,
) {
    Column(Modifier.fillMaxSize().padding(padding)) {
        Peer(model.alice, Modifier.weight(1f))
        HorizontalDivider()
        Peer(model.bob, Modifier.weight(1f))
    }
}

@Composable
private fun Peer(
    model: EditorModel,
    modifier: Modifier,
) {
    Column(modifier) {
        Text(
            model.label,
            style = MaterialTheme.typography.labelMedium,
            modifier = Modifier.padding(start = 20.dp, top = 12.dp),
        )
        AndroidView(factory = { model.view }, modifier = Modifier.fillMaxSize())
    }
}

private const val MIC = "🎙"

/** How long the listening mic takes to fade out, and again to fade back in. */
private const val BREATH_MILLIS = 700

/** The bottom bar's tabs: a title and an icon. */
private val TABS = listOf("Editor" to "✎", "Collaborate" to "⇄")

private val WELCOME =
    """
    # Welcome to canvist

    This text is laid out by **Rust** and painted on an Android *Canvas*. Select it, format it, and undo it like any other text.

    - [x] Type with Gboard, voice typing, or a hardware keyboard
    - [ ] Try `##` and a space for a heading

    > Every change is a step that collaborators merge.
    """.trimIndent()
