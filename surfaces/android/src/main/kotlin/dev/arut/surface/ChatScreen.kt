package dev.arut.surface

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Menu
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
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.arut.ffi.ChatMessage
import dev.arut.ffi.ChatRole
import dev.arut.ffi.ChatSummary
import java.text.DateFormat
import java.util.Date
import kotlinx.coroutines.launch

@Composable
fun ChatScreen(viewModel: ConversationViewModel) {
    val conversations by viewModel.conversations.collectAsStateWithLifecycle()
    val transcript by viewModel.transcript.collectAsStateWithLifecycle()
    val draft by viewModel.draft.collectAsStateWithLifecycle()
    val composerFailure by viewModel.composerFailure.collectAsStateWithLifecycle()
    val failure = transcript.failure ?: composerFailure

    val title = conversations.summaries.firstOrNull { it.id == conversations.selectedId }?.title

    // Material's own adaptive rule: a permanent list beside the conversation
    // once there is room for both, a modal one when there is not.
    BoxWithConstraints(Modifier.fillMaxSize()) {
        if (maxWidth >= 840.dp) {
            PermanentNavigationDrawer(
                drawerContent = {
                    PermanentDrawerSheet(Modifier.width(320.dp)) {
                        ConversationList(
                            summaries = conversations.summaries,
                            selectedId = conversations.selectedId,
                            onSelect = viewModel::select,
                            onNew = viewModel::newConversation,
                        )
                    }
                }
            ) {
                Conversation(
                    transcript = transcript,
                    draft = draft,
                    failure = failure,
                    title = title,
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
                    ModalDrawerSheet {
                        ConversationList(
                            summaries = conversations.summaries,
                            selectedId = conversations.selectedId,
                            onSelect = { id ->
                                viewModel.select(id)
                                scope.launch { drawer.close() }
                            },
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
                    title = title,
                    onOpenList = { scope.launch { drawer.open() } },
                    onEdit = viewModel::edit,
                    onSend = viewModel::send,
                )
            }
        }
    }
}

@Composable
private fun ConversationList(
    summaries: List<ChatSummary>,
    selectedId: String?,
    onSelect: (String?) -> Unit,
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
        HorizontalDivider()
        if (summaries.isEmpty()) {
            Text(
                text = stringResource(R.string.chat_history_empty),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(16.dp),
            )
            return@Column
        }
        LazyColumn {
            items(summaries, key = { it.id }) { summary ->
                NavigationDrawerItem(
                    label = { Text(summary.title, maxLines = 2, overflow = TextOverflow.Ellipsis) },
                    selected = summary.id == selectedId,
                    onClick = { onSelect(summary.id) },
                )
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Conversation(
    transcript: Transcript,
    draft: String,
    failure: Failure?,
    title: String?,
    onOpenList: (() -> Unit)?,
    onEdit: (String) -> Unit,
    onSend: () -> Unit,
) {
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
        }
    ) { insets ->
        Column(Modifier.padding(insets).fillMaxSize()) {
            Box(Modifier.weight(1f)) {
                if (transcript.messages.isEmpty()) {
                    EmptyConversation()
                } else {
                    Messages(transcript.messages)
                }
            }
            if (failure != null) {
                Text(
                    text = localized(failure.key, failure.arguments),
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(horizontal = 16.dp),
                )
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
private fun Messages(messages: List<ChatMessage>) {
    val scroll = rememberLazyListState()
    LaunchedEffect(messages.lastOrNull()?.id) {
        if (messages.isNotEmpty()) scroll.animateScrollToItem(messages.lastIndex)
    }
    LazyColumn(
        state = scroll,
        modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        items(messages, key = { it.id }) { message -> MessageBubble(message) }
    }
}

@Composable
private fun MessageBubble(message: ChatMessage) {
    val outgoing = message.role == ChatRole.USER
    Column(Modifier.fillMaxWidth()) {
        if (message.startsTimeGroup && message.acceptedAtMs > 0uL) {
            Text(
                text = timeOf(message.acceptedAtMs),
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
                modifier = Modifier.widthIn(max = 560.dp),
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
    Row(
        Modifier.fillMaxWidth().padding(12.dp),
        verticalAlignment = Alignment.Bottom,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        OutlinedTextField(
            value = draft,
            onValueChange = onEdit,
            modifier = Modifier.weight(1f),
            enabled = !isSending,
            placeholder = { Text(stringResource(R.string.composer_placeholder)) },
            label = { Text(stringResource(R.string.label_draft)) },
            shape = RoundedCornerShape(24.dp),
            maxLines = 7,
        )
        IconButton(onClick = onSend, enabled = canSend) {
            if (isSending) {
                CircularProgressIndicator(Modifier.width(20.dp))
            } else {
                Icon(
                    Icons.AutoMirrored.Filled.Send,
                    contentDescription = stringResource(R.string.action_send_message),
                )
            }
        }
    }
}

private fun timeOf(acceptedAtMs: ULong): String =
    DateFormat.getTimeInstance(DateFormat.SHORT).format(Date(acceptedAtMs.toLong()))

@Preview
@Composable
private fun EmptyConversationPreview() {
    ArutTheme { EmptyConversation() }
}
