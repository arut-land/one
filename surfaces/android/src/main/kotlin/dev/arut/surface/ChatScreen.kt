package dev.arut.surface

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Badge
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.NavigationDrawerItem
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.PermanentDrawerSheet
import androidx.compose.material3.PermanentNavigationDrawer
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SmallFloatingActionButton
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.window.core.layout.WindowSizeClass
// Strings are the binding library's resources (ADR 0022); AGP's R classes are
// non-transitive, so the app names that R rather than its own.
import dev.arut.bindings.R
import dev.arut.bindings.ErrorSource
import dev.arut.bindings.acceptedAt
import dev.arut.bindings.generated.Messages
import dev.arut.bindings.localized
import dev.arut.ffi.ChatMessage
import dev.arut.ffi.ChatRole
import dev.arut.ffi.ChatSummary
import java.text.DateFormat
import java.util.Calendar
import java.util.Date
import kotlinx.coroutines.launch

/**
 * Binds the view model to the screen. The screen itself takes plain state and
 * one stable set of actions, so it previews and tests without a view model.
 */
@Composable
fun ChatRoute(viewModel: ConversationViewModel, windowSizeClass: WindowSizeClass) {
    val conversations by viewModel.conversations.collectAsStateWithLifecycle()
    val transcript by viewModel.transcript.collectAsStateWithLifecycle()
    val draft by viewModel.draft.collectAsStateWithLifecycle()
    val composerFailure by viewModel.composerFailure.collectAsStateWithLifecycle()
    val actions = remember(viewModel) { ChatActions(viewModel) }
    ChatScreen(
        conversations = conversations,
        transcript = transcript,
        draft = draft,
        failure = transcript.failure ?: composerFailure,
        actions = actions,
        windowSizeClass = windowSizeClass,
    )
}

/** The screen's intents, one instance per view model so no lambda is recreated per frame. */
@Stable
class ChatActions(viewModel: ConversationViewModel) {
    val select: (String?) -> Unit = viewModel::select
    val search: (String) -> Unit = viewModel::search
    val newConversation: () -> Unit = viewModel::newConversation
    val rename: (String, String) -> Unit = viewModel::rename
    val delete: (String) -> Unit = viewModel::delete
    val edit: (String) -> Unit = viewModel::edit
    val send: () -> Unit = viewModel::send
}

@Composable
fun ChatScreen(
    conversations: Conversations,
    transcript: Transcript,
    draft: String,
    failure: ErrorSource?,
    actions: ChatActions,
    windowSizeClass: WindowSizeClass,
) {
    // Material's own adaptive rule, read off the window rather than measured by
    // us: a permanent list beside the conversation once there is room for both.
    val expanded =
        windowSizeClass.isWidthAtLeastBreakpoint(WindowSizeClass.WIDTH_DP_EXPANDED_LOWER_BOUND)
    val drawer = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    val closeList: () -> Unit = {
        if (!expanded) scope.launch { drawer.close() }
    }
    val list: @Composable () -> Unit = {
        ConversationList(
            conversations = conversations,
            onSelect = { id ->
                actions.select(id)
                closeList()
            },
            onSearch = actions.search,
            onRename = actions.rename,
            onDelete = actions.delete,
            onNew = {
                actions.newConversation()
                closeList()
            },
        )
    }
    val conversation: @Composable () -> Unit = {
        Conversation(
            transcript = transcript,
            draft = draft,
            failure = failure,
            title = conversations.title,
            onOpenList = if (expanded) null else ({ scope.launch { drawer.open() } }),
            onEdit = actions.edit,
            onSend = actions.send,
        )
    }
    if (expanded) {
        PermanentNavigationDrawer(
            drawerContent = { PermanentDrawerSheet(Modifier.width(320.dp)) { list() } },
            content = conversation,
        )
    } else {
        ModalNavigationDrawer(
            drawerState = drawer,
            // A sheet given the drawer state handles back itself, and animates
            // through a predictive back gesture on Android 14+.
            drawerContent = { ModalDrawerSheet(drawerState = drawer) { list() } },
            content = conversation,
        )
    }
}

@Composable
private fun ConversationList(
    conversations: Conversations,
    onSelect: (String?) -> Unit,
    onSearch: (String) -> Unit,
    onRename: (String, String) -> Unit,
    onDelete: (String) -> Unit,
    onNew: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var renameTarget by remember { mutableStateOf<ChatSummary?>(null) }
    var deleteTarget by remember { mutableStateOf<ChatSummary?>(null) }
    Column(modifier.padding(horizontal = 12.dp)) {
        TextButton(onClick = onNew, modifier = Modifier.fillMaxWidth()) {
            Icon(Icons.Filled.Add, contentDescription = null)
            Text(
                text = stringResource(R.string.action_new_conversation),
                modifier = Modifier.padding(start = 8.dp),
            )
        }
        // Rust narrows the list; this field only carries the query to it.
        OutlinedTextField(
            value = conversations.query,
            onValueChange = onSearch,
            modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp),
            placeholder = { Text(stringResource(R.string.conversation_search_placeholder)) },
            singleLine = true,
            shape = RoundedCornerShape(24.dp),
        )
        HorizontalDivider()
        if (conversations.summaries.isEmpty()) {
            Text(
                text =
                    if (conversations.query.isEmpty()) {
                        stringResource(R.string.chat_history_empty)
                    } else {
                        stringResource(R.string.conversation_search_empty)
                    },
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(16.dp),
            )
            return@Column
        }
        LazyColumn {
            items(conversations.summaries, key = { it.id }, contentType = { it.unread }) { summary ->
                ConversationRow(
                    summary = summary,
                    selected = summary.id == conversations.selectedId,
                    onSelect = onSelect,
                    onRename = { renameTarget = summary },
                    onDelete = { deleteTarget = summary },
                    modifier = Modifier.animateItem(),
                )
            }
        }
    }
    renameTarget?.let { summary ->
        RenameConversationDialog(
            summary = summary,
            onDismiss = { renameTarget = null },
            onRename = { title ->
                onRename(summary.id, title)
                renameTarget = null
            },
        )
    }
    deleteTarget?.let { summary ->
        DeleteConversationDialog(
            onDismiss = { deleteTarget = null },
            onDelete = {
                onDelete(summary.id)
                deleteTarget = null
            },
        )
    }
}

@Composable
private fun ConversationRow(
    summary: ChatSummary,
    selected: Boolean,
    onSelect: (String?) -> Unit,
    onRename: () -> Unit,
    onDelete: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var menuExpanded by remember { mutableStateOf(false) }
    val unread = stringResource(R.string.label_unread_messages)
    val description = if (summary.unread) "${summary.title}. $unread" else summary.title
    NavigationDrawerItem(
        label = { Text(summary.title, maxLines = 2, overflow = TextOverflow.Ellipsis) },
        badge = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                // Rust decides what is unread; the badge only draws it.
                if (summary.unread) Badge()
                Box {
                    IconButton(onClick = { menuExpanded = true }) {
                        Icon(
                            Icons.Filled.MoreVert,
                            contentDescription =
                                stringResource(R.string.action_conversation_options, summary.title),
                        )
                    }
                    DropdownMenu(
                        expanded = menuExpanded,
                        onDismissRequest = { menuExpanded = false },
                    ) {
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.action_rename_conversation)) },
                            onClick = {
                                menuExpanded = false
                                onRename()
                            },
                        )
                        DropdownMenuItem(
                            text = {
                                Text(
                                    stringResource(R.string.action_delete_conversation),
                                    color = MaterialTheme.colorScheme.error,
                                )
                            },
                            onClick = {
                                menuExpanded = false
                                onDelete()
                            },
                        )
                    }
                }
            }
        },
        selected = selected,
        onClick = { onSelect(summary.id) },
        modifier = modifier.semantics { contentDescription = description },
    )
}

@Composable
private fun RenameConversationDialog(
    summary: ChatSummary,
    onDismiss: () -> Unit,
    onRename: (String) -> Unit,
) {
    var title by rememberSaveable(summary.id) { mutableStateOf(summary.title) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.conversation_rename_title)) },
        text = {
            OutlinedTextField(
                value = title,
                onValueChange = { title = it },
                label = {
                    Text(stringResource(R.string.label_conversation_title))
                },
                singleLine = true,
            )
        },
        confirmButton = {
            TextButton(onClick = { onRename(title) }, enabled = title.isNotBlank()) {
                Text(stringResource(R.string.action_save))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.action_cancel)) }
        },
    )
}

@Composable
private fun DeleteConversationDialog(onDismiss: () -> Unit, onDelete: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.conversation_delete_title)) },
        text = { Text(stringResource(R.string.conversation_delete_message)) },
        confirmButton = {
            TextButton(onClick = onDelete) {
                Text(
                    stringResource(R.string.action_delete_conversation),
                    color = MaterialTheme.colorScheme.error,
                )
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.action_cancel)) }
        },
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Conversation(
    transcript: Transcript,
    draft: String,
    failure: ErrorSource?,
    title: String?,
    onOpenList: (() -> Unit)?,
    onEdit: (String) -> Unit,
    onSend: () -> Unit,
) {
    val context = LocalContext.current
    val snackbars = remember { SnackbarHostState() }
    // The core names an error by Fluent id; the generated map turns it into
    // this locale's sentence, and Material puts a transient one in a Snackbar.
    val message = failure?.localized(context, Messages.byKey)
    LaunchedEffect(message) {
        if (message != null) snackbars.showSnackbar(message)
    }
    Scaffold(
        // The composer takes the bottom edge itself, so the keyboard and the
        // navigation bar are counted once, as the larger of the two.
        contentWindowInsets =
            WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal + WindowInsetsSides.Top),
        topBar = {
            TopAppBar(
                title = { Text(title ?: stringResource(R.string.action_new_conversation)) },
                navigationIcon = {
                    if (onOpenList != null) {
                        IconButton(onClick = onOpenList) {
                            Icon(
                                Icons.Filled.Menu,
                                contentDescription = stringResource(R.string.label_conversations),
                            )
                        }
                    }
                },
            )
        },
        snackbarHost = { SnackbarHost(snackbars) },
    ) { insets ->
        Column(Modifier.padding(insets).fillMaxSize()) {
            Box(Modifier.weight(1f)) {
                if (transcript.messages.isEmpty()) {
                    EmptyConversation()
                } else {
                    MessageList(transcript.messages)
                }
            }
            Composer(
                draft = draft,
                canSend = transcript.state?.canSend == true && draft.isNotBlank(),
                isSending = transcript.isSending,
                onEdit = onEdit,
                onSend = onSend,
            )
        }
    }
}

@Composable
private fun MessageList(messages: List<ChatMessage>) {
    val context = LocalContext.current
    // The platform formatter follows the person's 12- or 24-hour setting and
    // is built once for the list rather than once per bubble.
    val timeFormat = remember(context) { android.text.format.DateFormat.getTimeFormat(context) }
    val dateFormat = remember(context) { android.text.format.DateFormat.getMediumDateFormat(context) }
    val scroll = rememberLazyListState()
    val scope = rememberCoroutineScope()
    val nearBottom by remember(scroll) { derivedStateOf { scroll.isNearBottom() } }
    // Only the newest reply is announced; a live region on the whole list would
    // read the transcript back on every change.
    val latestIncoming = messages.lastOrNull { it.role != ChatRole.USER }?.id
    // New activity follows the tail only while the reader is already there.
    // Compose scales every animation by the system's animator duration, so a
    // device with motion turned off lands on the last message in one frame.
    LaunchedEffect(messages.lastOrNull()?.id) {
        if (messages.isNotEmpty() && nearBottom) {
            scroll.animateScrollToItem(messages.lastIndex)
        }
    }
    Box(Modifier.fillMaxSize()) {
        LazyColumn(
            state = scroll,
            modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            // The two speakers are two row shapes, so they recycle separately.
            items(messages, key = { it.id }, contentType = { it.role }) { message ->
                MessageBubble(
                    message = message,
                    announced = message.id == latestIncoming,
                    timeFormat = timeFormat,
                    dateFormat = dateFormat,
                    modifier = Modifier.animateItem(),
                )
            }
        }
        AnimatedVisibility(
            visible = !nearBottom,
            modifier = Modifier.align(Alignment.BottomEnd).padding(16.dp),
        ) {
            SmallFloatingActionButton(
                onClick = { scope.launch { scroll.animateScrollToItem(messages.lastIndex) } }
            ) {
                Icon(
                    Icons.Filled.KeyboardArrowDown,
                    contentDescription = stringResource(R.string.action_scroll_to_latest),
                )
            }
        }
    }
}

@Composable
private fun MessageBubble(
    message: ChatMessage,
    announced: Boolean,
    timeFormat: DateFormat,
    dateFormat: DateFormat,
    modifier: Modifier = Modifier,
) {
    val outgoing = message.role == ChatRole.USER
    val speaker =
        if (outgoing) {
            stringResource(R.string.chat_role_you)
        } else {
            stringResource(R.string.chat_role_assistant)
        }
    // Rust supplies the instants; Android supplies the local date and time words.
    val accepted = acceptedAt(message.acceptedAtMs)
    val time = accepted?.let(timeFormat::format).orEmpty()
    val groupStamp =
        if (!message.startsTimeGroup) {
            ""
        } else {
            val previous = message.previousTimeGroupAtMs?.let(::acceptedAt)
            accepted?.let { date ->
                if (previous == null || !date.isSameLocalDay(previous)) {
                    "${dateFormat.format(date)}, ${timeFormat.format(date)}"
                } else {
                    timeFormat.format(date)
                }
            }.orEmpty()
        }
    val description =
        if (time.isEmpty()) "$speaker: ${message.text}" else "$speaker: ${message.text}. $time"
    // Rust says where a speaker's run ends; this only spaces after it.
    Column(
        modifier.fillMaxWidth().padding(bottom = if (message.endsSpeakerGroup) 12.dp else 0.dp)
    ) {
        if (message.startsTimeGroup && groupStamp.isNotEmpty()) {
            Text(
                text = groupStamp,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.align(Alignment.CenterHorizontally).padding(vertical = 8.dp),
            )
        }
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = if (outgoing) Arrangement.End else Arrangement.Start,
        ) {
            Surface(
                color =
                    if (outgoing) MaterialTheme.colorScheme.primaryContainer
                    else MaterialTheme.colorScheme.surfaceVariant,
                contentColor =
                    if (outgoing) MaterialTheme.colorScheme.onPrimaryContainer
                    else MaterialTheme.colorScheme.onSurfaceVariant,
                shape = RoundedCornerShape(16.dp),
                // One traversal unit per bubble: speaker, text and time together.
                modifier =
                    Modifier.widthIn(max = 560.dp).semantics(mergeDescendants = true) {
                        contentDescription = description
                        if (announced) liveRegion = LiveRegionMode.Polite
                    },
            ) {
                Text(
                    text = message.text,
                    style = MaterialTheme.typography.bodyLarge,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
                )
            }
        }
    }
}

@Composable
private fun EmptyConversation() {
    Column(
        Modifier.fillMaxSize().padding(32.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            text = stringResource(R.string.chat_empty_title),
            style = MaterialTheme.typography.headlineSmall,
        )
        Text(
            text = stringResource(R.string.chat_empty_hint),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 8.dp),
        )
    }
}

/**
 * The editor owns its text as `TextFieldState`, so the cursor and an IME
 * composition survive Rust echoing the draft back; the echo lands only when it
 * differs, which `Draft` already guarantees while a keystroke is unacknowledged.
 */
@Composable
private fun Composer(
    draft: String,
    canSend: Boolean,
    isSending: Boolean,
    onEdit: (String) -> Unit,
    onSend: () -> Unit,
) {
    val state = rememberTextFieldState(draft)
    val edit by rememberUpdatedState(onEdit)
    LaunchedEffect(state) { snapshotFlow { state.text.toString() }.collect { edit(it) } }
    LaunchedEffect(draft) {
        if (draft != state.text.toString()) state.setTextAndPlaceCursorAtEnd(draft)
    }
    Row(
        // The bottom edge is the keyboard or the navigation bar, whichever is
        // taller; the scaffold above left it to this row.
        Modifier.fillMaxWidth()
            .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom))
            .padding(12.dp),
        verticalAlignment = Alignment.Bottom,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        OutlinedTextField(
            state = state,
            modifier =
                Modifier.weight(1f).onPreviewKeyEvent { event ->
                    // A hardware Enter sends; Shift+Enter keeps the newline.
                    val sends =
                        event.type == KeyEventType.KeyDown &&
                            event.key == Key.Enter &&
                            !event.isShiftPressed
                    if (sends && canSend) onSend()
                    sends
                },
            // Placeholder only: a floating label permanently consumes one of the
            // seven lines this composer has.
            placeholder = { Text(stringResource(R.string.composer_placeholder)) },
            shape = RoundedCornerShape(24.dp),
            lineLimits = TextFieldLineLimits.MultiLine(maxHeightInLines = 7),
            keyboardOptions =
                KeyboardOptions(
                    capitalization = KeyboardCapitalization.Sentences,
                    imeAction = ImeAction.Send,
                ),
            onKeyboardAction = { if (canSend) onSend() },
        )
        IconButton(onClick = onSend, enabled = canSend) {
            if (isSending) {
                CircularProgressIndicator(Modifier.size(20.dp))
            } else {
                Icon(
                    Icons.AutoMirrored.Filled.Send,
                    contentDescription = stringResource(R.string.action_send_message),
                )
            }
        }
    }
}

private fun LazyListState.isNearBottom(): Boolean {
    val last = layoutInfo.visibleItemsInfo.lastOrNull() ?: return true
    return last.index >= layoutInfo.totalItemsCount - 1
}

private fun Date.isSameLocalDay(other: Date): Boolean {
    val first = Calendar.getInstance().apply { time = this@isSameLocalDay }
    val second = Calendar.getInstance().apply { time = other }
    return first.get(Calendar.ERA) == second.get(Calendar.ERA) &&
        first.get(Calendar.YEAR) == second.get(Calendar.YEAR) &&
        first.get(Calendar.DAY_OF_YEAR) == second.get(Calendar.DAY_OF_YEAR)
}

@Preview
@Composable
private fun EmptyConversationPreview() {
    ArutTheme { EmptyConversation() }
}

@Preview
@Composable
private fun DeleteConversationDialogPreview() {
    ArutTheme { DeleteConversationDialog(onDismiss = {}, onDelete = {}) }
}
