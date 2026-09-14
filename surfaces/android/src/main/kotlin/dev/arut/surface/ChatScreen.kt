package dev.arut.surface

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ArrowDownward
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material3.Badge
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DrawerValue
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
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.window.core.layout.WindowSizeClass
import dev.arut.bindings.ErrorSource
import dev.arut.bindings.acceptedAt
import dev.arut.bindings.generated.Messages
import dev.arut.bindings.localized
import dev.arut.ffi.ChatMessage
import dev.arut.ffi.ChatRole
import dev.arut.ffi.ChatSummary
import java.text.DateFormat
import kotlinx.coroutines.launch

@Composable
fun ChatScreen(viewModel: ConversationViewModel, windowSizeClass: WindowSizeClass) {
    val conversations by viewModel.conversations.collectAsStateWithLifecycle()
    val transcript by viewModel.transcript.collectAsStateWithLifecycle()
    val draft by viewModel.draft.collectAsStateWithLifecycle()
    val composerFailure by viewModel.composerFailure.collectAsStateWithLifecycle()
    val failure = transcript.failure ?: composerFailure

    // Material's own adaptive rule, read off the window rather than measured by
    // us: a permanent list beside the conversation once there is room for both.
    if (windowSizeClass.isWidthAtLeastBreakpoint(WindowSizeClass.WIDTH_DP_EXPANDED_LOWER_BOUND)) {
        PermanentNavigationDrawer(
            drawerContent = {
                PermanentDrawerSheet(Modifier.width(320.dp)) {
                    ConversationList(
                        conversations = conversations,
                        onSelect = viewModel::select,
                        onSearch = viewModel::search,
                        onNew = viewModel::newConversation,
                    )
                }
            }
        ) {
            Conversation(
                transcript = transcript,
                draft = draft,
                failure = failure,
                title = conversations.title,
                onOpenList = null,
                onEdit = viewModel::edit,
                onSend = viewModel::send,
            )
        }
    } else {
        val drawer = rememberDrawerState(DrawerValue.Closed)
        val scope = rememberCoroutineScope()
        ModalNavigationDrawer(
            drawerState = drawer,
            drawerContent = {
                // A sheet given the drawer state handles back itself, and
                // animates through a predictive back gesture on Android 14+.
                ModalDrawerSheet(drawerState = drawer) {
                    ConversationList(
                        conversations = conversations,
                        onSelect = { id ->
                            viewModel.select(id)
                            scope.launch { drawer.close() }
                        },
                        onSearch = viewModel::search,
                        onNew = {
                            viewModel.newConversation()
                            scope.launch { drawer.close() }
                        },
                    )
                }
            },
        ) {
            Conversation(
                transcript = transcript,
                draft = draft,
                failure = failure,
                title = conversations.title,
                onOpenList = { scope.launch { drawer.open() } },
                onEdit = viewModel::edit,
                onSend = viewModel::send,
            )
        }
    }
}

@Composable
private fun ConversationList(
    conversations: Conversations,
    onSelect: (String?) -> Unit,
    onSearch: (String) -> Unit,
    onNew: () -> Unit,
    modifier: Modifier = Modifier,
) {
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
            items(conversations.summaries, key = { it.id }) { summary ->
                ConversationRow(
                    summary = summary,
                    selected = summary.id == conversations.selectedId,
                    onSelect = onSelect,
                )
            }
        }
    }
}

@Composable
private fun ConversationRow(summary: ChatSummary, selected: Boolean, onSelect: (String?) -> Unit) {
    val unread = stringResource(R.string.label_unread_messages)
    val description = if (summary.unread) "${summary.title}. $unread" else summary.title
    NavigationDrawerItem(
        label = { Text(summary.title, maxLines = 2, overflow = TextOverflow.Ellipsis) },
        // Rust decides what is unread; the badge only draws it.
        badge = if (summary.unread) ({ Badge() }) else null,
        selected = selected,
        onClick = { onSelect(summary.id) },
        modifier = Modifier.semantics { contentDescription = description },
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
    val scroll = rememberLazyListState()
    val scope = rememberCoroutineScope()
    val nearBottom by remember(scroll) { derivedStateOf { scroll.isNearBottom() } }
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
            modifier =
                Modifier.fillMaxSize()
                    .padding(horizontal = 16.dp)
                    .semantics { liveRegion = LiveRegionMode.Polite },
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            items(messages, key = { it.id }) { message -> MessageBubble(message) }
        }
        AnimatedVisibility(
            visible = !nearBottom,
            modifier = Modifier.align(Alignment.BottomEnd).padding(16.dp),
        ) {
            SmallFloatingActionButton(
                onClick = { scope.launch { scroll.animateScrollToItem(messages.lastIndex) } }
            ) {
                Icon(
                    Icons.Filled.ArrowDownward,
                    contentDescription = stringResource(R.string.action_scroll_to_latest),
                )
            }
        }
    }
}

@Composable
private fun MessageBubble(message: ChatMessage) {
    val outgoing = message.role == ChatRole.USER
    val speaker =
        if (outgoing) {
            stringResource(R.string.chat_role_you)
        } else {
            stringResource(R.string.chat_role_assistant)
        }
    val stamp = timeOf(message)
    val description =
        if (stamp.isEmpty()) "$speaker: ${message.text}" else "$speaker: ${message.text}. $stamp"
    // Rust says where a speaker's run ends; this only spaces after it.
    Column(
        Modifier.fillMaxWidth().padding(bottom = if (message.endsSpeakerGroup) 12.dp else 0.dp)
    ) {
        if (message.startsTimeGroup && stamp.isNotEmpty()) {
            Text(
                text = stamp,
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
                modifier =
                    Modifier.widthIn(max = 560.dp).semantics {
                        contentDescription = description
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

@Composable
private fun Composer(
    draft: String,
    canSend: Boolean,
    isSending: Boolean,
    onEdit: (String) -> Unit,
    onSend: () -> Unit,
) {
    val label = stringResource(R.string.label_draft)
    Row(
        // `adjustResize` plus these insets keep the field above the keyboard and
        // the navigation bar without a fixed margin.
        Modifier.fillMaxWidth().imePadding().navigationBarsPadding().padding(12.dp),
        verticalAlignment = Alignment.Bottom,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        OutlinedTextField(
            value = draft,
            onValueChange = onEdit,
            modifier = Modifier.weight(1f).semantics { contentDescription = label },
            // Placeholder only: a floating label permanently consumes one of the
            // seven lines this composer has.
            placeholder = { Text(stringResource(R.string.composer_placeholder)) },
            shape = RoundedCornerShape(24.dp),
            maxLines = 7,
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

/** Rust supplies the instant; Android supplies the words. */
private fun timeOf(message: ChatMessage): String =
    acceptedAt(message.acceptedAtMs)?.let { DateFormat.getTimeInstance(DateFormat.SHORT).format(it) }
        ?: ""

private fun LazyListState.isNearBottom(): Boolean {
    val last = layoutInfo.visibleItemsInfo.lastOrNull() ?: return true
    return last.index >= layoutInfo.totalItemsCount - 1
}

@Preview
@Composable
private fun EmptyConversationPreview() {
    ArutTheme { EmptyConversation() }
}
