package dev.arut.surface

import android.os.Build
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.weight
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.outlined.ChatBubbleOutline
import androidx.compose.material3.CenterAlignedTopAppBar
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.FilledIconButton
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
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.arut.bindings.ChatModel
import dev.arut.bindings.ChatRole
import dev.arut.bindings.ChatState
import dev.arut.bindings.ChatStatus
import dev.arut.bindings.ChatSummary
import kotlinx.coroutines.launch

private val WideNavigationWidth = 840.dp
private val DrawerWidth = 304.dp
private val ContentWidth = 840.dp

@Composable
fun ChatScreen(model: ChatModel) {
    val input by model.input.collectAsState()
    val state by model.state.collectAsState()
    val history by model.history.collectAsState()
    val selectedChatId by model.selectedChatId.collectAsState()
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    val composerFocusRequester = remember { FocusRequester() }
    val context = LocalContext.current
    val darkTheme = isSystemInDarkTheme()
    val colorScheme = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        if (darkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
    } else if (darkTheme) {
        darkColorScheme()
    } else {
        lightColorScheme()
    }

    MaterialTheme(colorScheme = colorScheme) {
        BoxWithConstraints(modifier = Modifier.fillMaxSize()) {
            if (maxWidth >= WideNavigationWidth) {
                PermanentNavigationDrawer(
                    drawerContent = {
                        PermanentDrawerSheet(modifier = Modifier.width(DrawerWidth)) {
                            ChatDrawer(
                                history = history,
                                selectedChatId = selectedChatId,
                                onNewChat = {
                                    model.newChat()
                                    composerFocusRequester.requestFocus()
                                },
                                onSelectChat = model::selectChat,
                            )
                        }
                    },
                ) {
                    Conversation(
                        input = input,
                        state = state,
                        title = history.firstOrNull { it.id == selectedChatId }?.title ?: "New conversation",
                        showNavigation = false,
                        onNavigationClick = {},
                        onNewChat = {
                            model.newChat()
                            composerFocusRequester.requestFocus()
                        },
                        onInputChange = model::setInput,
                        onSend = model::send,
                        composerFocusRequester = composerFocusRequester,
                    )
                }
            } else {
                BackHandler(enabled = drawerState.isOpen) {
                    scope.launch { drawerState.close() }
                }
                ModalNavigationDrawer(
                    drawerState = drawerState,
                    gesturesEnabled = true,
                    drawerContent = {
                        ModalDrawerSheet(modifier = Modifier.width(DrawerWidth)) {
                            ChatDrawer(
                                history = history,
                                selectedChatId = selectedChatId,
                                onNewChat = {
                                    model.newChat()
                                    scope.launch {
                                        drawerState.close()
                                        composerFocusRequester.requestFocus()
                                    }
                                },
                                onSelectChat = { chatId ->
                                    model.selectChat(chatId)
                                    scope.launch { drawerState.close() }
                                },
                            )
                        }
                    },
                ) {
                    Conversation(
                        input = input,
                        state = state,
                        title = history.firstOrNull { it.id == selectedChatId }?.title ?: "New conversation",
                        showNavigation = true,
                        onNavigationClick = { scope.launch { drawerState.open() } },
                        onNewChat = {
                            model.newChat()
                            composerFocusRequester.requestFocus()
                        },
                        onInputChange = model::setInput,
                        onSend = model::send,
                        composerFocusRequester = composerFocusRequester,
                    )
                }
            }
        }
    }
}

@Composable
private fun ChatDrawer(
    history: List<ChatSummary>,
    selectedChatId: String,
    onNewChat: () -> Unit,
    onSelectChat: (String) -> Unit,
) {
    Column(modifier = Modifier.fillMaxHeight()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(start = 24.dp, end = 12.dp, top = 20.dp, bottom = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Surface(
                modifier = Modifier.size(34.dp),
                shape = RoundedCornerShape(11.dp),
                color = MaterialTheme.colorScheme.primary,
            ) {
                Box(contentAlignment = Alignment.Center) {
                    Text(
                        "A",
                        color = MaterialTheme.colorScheme.onPrimary,
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.Bold,
                    )
                }
            }
            Text(
                "Arut",
                modifier = Modifier.padding(start = 12.dp),
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.SemiBold,
            )
        }

        NavigationDrawerItem(
            label = { Text("New chat", fontWeight = FontWeight.Medium) },
            icon = { Icon(Icons.Default.Add, contentDescription = null) },
            selected = false,
            onClick = onNewChat,
            modifier = Modifier.padding(horizontal = 12.dp),
        )

        Text(
            "RECENT",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            fontWeight = FontWeight.SemiBold,
            modifier = Modifier.padding(start = 28.dp, top = 22.dp, bottom = 8.dp),
        )

        LazyColumn(
            modifier = Modifier.fillMaxWidth().weight(1f),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            items(history, key = { it.id }) { chat ->
                NavigationDrawerItem(
                    label = {
                        Text(chat.title, maxLines = 2, overflow = TextOverflow.Ellipsis)
                    },
                    selected = selectedChatId == chat.id,
                    icon = { Icon(Icons.Outlined.ChatBubbleOutline, contentDescription = null) },
                    onClick = { onSelectChat(chat.id) },
                    modifier = Modifier.padding(horizontal = 12.dp),
                    shape = RoundedCornerShape(14.dp),
                )
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Conversation(
    input: String,
    state: ChatState,
    title: String,
    showNavigation: Boolean,
    onNavigationClick: () -> Unit,
    onNewChat: () -> Unit,
    onInputChange: (String) -> Unit,
    onSend: () -> Unit,
    composerFocusRequester: FocusRequester,
) {
    Scaffold(
        containerColor = MaterialTheme.colorScheme.surfaceContainerLowest,
        topBar = {
            CenterAlignedTopAppBar(
                title = {
                    Text(title, maxLines = 1, overflow = TextOverflow.Ellipsis)
                },
                navigationIcon = {
                    if (showNavigation) {
                        IconButton(
                            onClick = onNavigationClick,
                        ) {
                            Icon(Icons.Default.Menu, contentDescription = "Open chats")
                        }
                    }
                },
                actions = {
                    IconButton(onClick = onNewChat) {
                        Icon(Icons.Default.Add, contentDescription = "New chat")
                    }
                },
            )
        },
        bottomBar = {
            Composer(
                input = input,
                isSending = state.status == ChatStatus.SENDING,
                error = state.error,
                onInputChange = onInputChange,
                onSend = onSend,
                focusRequester = composerFocusRequester,
            )
        },
    ) { paddingValues ->
        val listState = rememberLazyListState()

        LaunchedEffect(state.messages.size) {
            if (state.messages.isNotEmpty()) {
                listState.animateScrollToItem(state.messages.lastIndex)
            }
        }

        if (state.messages.isEmpty()) {
            EmptyConversation(modifier = Modifier.fillMaxSize().padding(paddingValues))
        } else {
            LazyColumn(
                state = listState,
                modifier = Modifier.fillMaxSize().padding(paddingValues),
                contentPadding = PaddingValues(horizontal = 20.dp, vertical = 24.dp),
                verticalArrangement = Arrangement.spacedBy(18.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                items(state.messages, key = { it.id }) { message ->
                    val isUser = message.role == ChatRole.USER
                    Row(
                        modifier = Modifier.widthIn(max = ContentWidth).fillMaxWidth(),
                        horizontalArrangement = if (isUser) Arrangement.End else Arrangement.Start,
                    ) {
                        Column(
                            horizontalAlignment = if (isUser) Alignment.End else Alignment.Start,
                        ) {
                            Text(
                                if (isUser) "You" else "Arut",
                                style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.padding(horizontal = 4.dp, bottom = 5.dp),
                            )
                            Surface(
                                shape = RoundedCornerShape(20.dp),
                                color = if (isUser) {
                                    MaterialTheme.colorScheme.primary
                                } else {
                                    MaterialTheme.colorScheme.surfaceContainerHigh
                                },
                                contentColor = if (isUser) {
                                    MaterialTheme.colorScheme.onPrimary
                                } else {
                                    MaterialTheme.colorScheme.onSurface
                                },
                            ) {
                                Text(
                                    message.text,
                                    style = MaterialTheme.typography.bodyLarge,
                                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun EmptyConversation(modifier: Modifier = Modifier) {
    Box(modifier = modifier, contentAlignment = Alignment.Center) {
        Column(
            modifier = Modifier.padding(horizontal = 32.dp).widthIn(max = 420.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Surface(
                modifier = Modifier.size(58.dp),
                shape = CircleShape,
                color = MaterialTheme.colorScheme.primaryContainer,
            ) {
                Box(contentAlignment = Alignment.Center) {
                    Text(
                        "A",
                        color = MaterialTheme.colorScheme.onPrimaryContainer,
                        style = MaterialTheme.typography.headlineSmall,
                        fontWeight = FontWeight.Bold,
                    )
                }
            }
            Spacer(modifier = Modifier.height(18.dp))
            Text(
                "Start a conversation",
                style = MaterialTheme.typography.headlineSmall,
                fontWeight = FontWeight.SemiBold,
                textAlign = TextAlign.Center,
            )
            Text(
                "Ask a question or share what you are working on.",
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(top = 8.dp),
            )
        }
    }
}

@Composable
private fun Composer(
    input: String,
    isSending: Boolean,
    error: String,
    onInputChange: (String) -> Unit,
    onSend: () -> Unit,
    focusRequester: FocusRequester,
) {
    Surface(tonalElevation = 3.dp, shadowElevation = 3.dp) {
        Column(modifier = Modifier.fillMaxWidth().navigationBarsPadding().imePadding()) {
            HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
            if (error.isNotEmpty()) {
                Text(
                    error,
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier
                        .widthIn(max = ContentWidth)
                        .fillMaxWidth()
                        .align(Alignment.CenterHorizontally)
                        .padding(horizontal = 20.dp, vertical = 8.dp),
                )
            }
            Row(
                modifier = Modifier
                    .widthIn(max = ContentWidth)
                    .fillMaxWidth()
                    .align(Alignment.CenterHorizontally)
                    .padding(horizontal = 16.dp, vertical = 12.dp),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                OutlinedTextField(
                    value = input,
                    onValueChange = onInputChange,
                    modifier = Modifier.weight(1f).focusRequester(focusRequester),
                    placeholder = { Text("Message Arut") },
                    shape = RoundedCornerShape(22.dp),
                    singleLine = true,
                    enabled = !isSending,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
                    keyboardActions = KeyboardActions(onSend = { onSend() }),
                )
                FilledIconButton(
                    onClick = onSend,
                    enabled = input.isNotBlank() && !isSending,
                    shape = RoundedCornerShape(18.dp),
                    modifier = Modifier.size(52.dp),
                ) {
                    if (isSending) {
                        CircularProgressIndicator(
                            modifier = Modifier.size(22.dp),
                            strokeWidth = 2.dp,
                            color = MaterialTheme.colorScheme.onPrimary,
                        )
                    } else {
                        Icon(Icons.AutoMirrored.Filled.Send, contentDescription = "Send message")
                    }
                }
            }
        }
    }
}
