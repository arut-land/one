package dev.arut.surface

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import dev.arut.bindings.ObservableState
import dev.arut.ffi.*
import kotlinx.coroutines.launch

@Composable
fun ChatScreen(session: ProductSessionHandle) {
    var selected by remember { mutableStateOf(session.chat()) }
    val list = remember { session.conversations() }
    val observer = remember { ObservableState(list::state) { callback ->
        val subscription = list.listChanges(callback)
        AutoCloseable { subscription.cancel() }
    } }
    DisposableEffect(observer) { onDispose { observer.close() } }
    val conversations by observer.state.collectAsState()
    Row {
        Column {
            Button(onClick = { selected = session.newChat() }) { Text("New conversation") }
            conversations.forEach { summary ->
                TextButton(onClick = { session.selectChat(summary.id)?.let { selected = it } }) { Text(summary.title) }
            }
        }
        key(selected) { ConversationView(selected) }
    }
}

@Composable
private fun ConversationView(chat: ChatHandle) {
    val composer = remember { chat.composer() }
    val transcript = remember { ObservableState(chat::state) { callback ->
        val subscription = chat.chatChanges(callback)
        AutoCloseable { subscription.cancel() }
    } }
    val draft = remember { ObservableState(composer::state) { callback ->
        val subscription = composer.composerChanges(callback)
        AutoCloseable { subscription.cancel() }
    } }
    DisposableEffect(chat) { onDispose { transcript.close(); draft.close() } }
    LaunchedEffect(composer) { composer.initialize(); composer.follow() }
    val state by transcript.state.collectAsState()
    val composing by draft.state.collectAsState()
    val scope = rememberCoroutineScope()
    Column {
        LazyColumn { items(state.messages, key = { it.id }) { Text(it.text) } }
        TextField(value = composing.text, onValueChange = { text -> scope.launch { composer.replace(text) } })
        Button(onClick = { scope.launch { chat.send(composer.state().text) } }) { Text("Send") }
    }
}
