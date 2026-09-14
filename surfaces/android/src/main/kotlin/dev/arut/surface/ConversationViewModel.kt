package dev.arut.surface

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dev.arut.ffi.ChatHandle
import dev.arut.ffi.ChatMessage
import dev.arut.ffi.ChatState
import dev.arut.ffi.ChatSummary
import dev.arut.ffi.ComposerHandle
import dev.arut.ffi.ProductSessionHandle
import dev.arut.ffi.chatChanges
import dev.arut.ffi.composerChanges
import dev.arut.ffi.listChanges
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.onStart
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

/** What the sidebar shows: the list Rust keeps, and which row it has selected. */
data class Conversations(
    val summaries: List<ChatSummary> = emptyList(),
    val selectedId: String? = null,
)

/** A Fluent message id and the arguments it takes. Never a sentence. */
data class Failure(val key: String, val arguments: List<String>)

/** What the transcript shows. */
data class Transcript(
    val state: ChatState? = null,
    val messages: List<ChatMessage> = emptyList(),
    val failure: Failure? = null,
) {
    val isSending: Boolean
        get() = state?.isSending ?: false
}

/**
 * One view model over the session's scope handles. Each projection stays its
 * own `StateFlow` because each is its own scope (ADR 0007), so a new handle
 * costs one more `stateIn` line and no new view model.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class ConversationViewModel(private val session: ProductSessionHandle) : ViewModel() {
    private class Handles(val chat: ChatHandle, val composer: ComposerHandle)

    private val list = session.conversations()
    private val opened = mutableMapOf<String?, Handles>()
    private val draftText = MutableStateFlow("")
    private val draftFailure = MutableStateFlow<Failure?>(null)
    private val current: MutableStateFlow<Handles>

    init {
        // `current` has to exist before `follow` starts collecting: the first
        // composer revision arrives on this thread, inside this constructor.
        val chat = session.chat()
        val handles = Handles(chat, chat.composer())
        opened[null] = handles
        current = MutableStateFlow(handles)
        follow(handles)
    }

    val conversations: StateFlow<Conversations> =
        list.listChanges()
            .map { read() }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000L), read())

    val transcript: StateFlow<Transcript> =
        current
            .flatMapLatest { handles -> transcriptOf(handles.chat) }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000L), Transcript())

    /** The visible draft. Rust echoes every write and coalesces the rest. */
    val draft: StateFlow<String> = draftText.asStateFlow()
    val composerFailure: StateFlow<Failure?> = draftFailure.asStateFlow()

    fun select(id: String?) {
        if (id == conversations.value.selectedId) return
        list.select(id)
        current.value = open(id)
    }

    fun newConversation() {
        opened.remove(null)
        val chat = session.newChat()
        val handles = Handles(chat, chat.composer())
        opened[null] = handles
        follow(handles)
        list.select(null)
        current.value = handles
    }

    fun edit(text: String) {
        // Echo locally, then write. `viewModelScope` dispatches on the main
        // thread, so the writes reach Rust in typing order and it keeps the last.
        draftText.value = text
        val composer = current.value.composer
        viewModelScope.launch { composer.replace(text) }
    }

    fun send() {
        val state = transcript.value.state
        val text = draftText.value
        if (state == null || !state.canSend || text.isBlank()) return
        val chat = current.value.chat
        viewModelScope.launch { chat.send(text) }
    }

    override fun onCleared() {
        opened.values.forEach { handles ->
            handles.composer.close()
            handles.chat.close()
        }
        opened.clear()
        list.close()
    }

    private fun read() = Conversations(list.state(), list.selectedId())

    private fun open(id: String?): Handles =
        opened.getOrPut(id) {
            val chat = if (id == null) session.chat() else session.selectChat(id) ?: session.chat()
            Handles(chat, chat.composer()).also { follow(it) }
        }

    private fun follow(handles: Handles) {
        viewModelScope.launch {
            handles.composer.initialize()
            handles.composer.follow()
        }
        viewModelScope.launch {
            val composer = handles.composer
            composer.composerChanges().onStart { emit(0uL) }.collect {
                if (handles !== current.value) return@collect
                val state = composer.state()
                if (state.text != draftText.value) draftText.value = state.text
                draftFailure.value = composer.errorKey()?.let { Failure(it, composer.errorArgs()) }
            }
        }
    }

    private fun transcriptOf(chat: ChatHandle) = flow {
        var rows = chat.messagesAfter(0uL)
        emit(snapshot(chat, rows))
        chat.chatChanges().collect {
            val added = chat.messagesAfter(rows.lastOrNull()?.id ?: 0uL)
            if (added.isNotEmpty()) rows = rows + added
            emit(snapshot(chat, rows))
        }
    }

    private fun snapshot(chat: ChatHandle, rows: List<ChatMessage>) =
        Transcript(
            chat.state(),
            rows,
            chat.errorKey()?.let { Failure(it, chat.errorArgs()) },
        )
}
