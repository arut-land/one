package dev.arut.surface

import androidx.compose.runtime.Immutable
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dev.arut.bindings.Draft
import dev.arut.bindings.ErrorSource
import dev.arut.bindings.errorSource
import dev.arut.bindings.following
import dev.arut.bindings.projection
import dev.arut.bindings.rows
import dev.arut.bindings.stateOf
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
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch

/**
 * What the sidebar shows. Rust narrows the list to the query, carries where each
 * title matched, marks what is unread and names the selected conversation, so
 * this holds no search predicate and no title rule of its own (ADR 0021).
 *
 * `@Immutable`: the lists inside come from Rust and are never mutated here, so
 * Compose may skip a screen whose state compares equal.
 */
@Immutable
data class Conversations(
    val summaries: List<ChatSummary> = emptyList(),
    val selectedId: String? = null,
    val title: String? = null,
    val query: String = "",
)

/** What the transcript shows. */
@Immutable
data class Transcript(
    val state: ChatState? = null,
    val messages: List<ChatMessage> = emptyList(),
    val failure: ErrorSource? = null,
) {
    val isSending: Boolean
        get() = state?.isSending ?: false
}

/**
 * One view model over the session's scope handles. Each projection stays its
 * own `StateFlow` because each is its own scope (ADR 0007), and every read of a
 * revision goes through `dev.arut.bindings`, so a new handle costs one more
 * `stateOf` line and no loop of its own.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class ConversationViewModel(private val session: ProductSessionHandle) : ViewModel() {
    private class Handles(val chat: ChatHandle, val composer: ComposerHandle) {
        val draft = Draft(composer.state().text) { composer.replace(it) }
        val followers = mutableListOf<Job>()

        fun close() {
            followers.forEach(Job::cancel)
            composer.close()
            chat.close()
        }
    }

    private val list = session.conversations()
    // Visited conversations keep their handles, so a return is a rebind and
    // not a reload; access order and a bound keep that from growing with the
    // history. Rust retains every draft regardless.
    private val opened = LinkedHashMap<String?, Handles>(16, 0.75f, true)
    private val draftFailure = MutableStateFlow<ErrorSource?>(null)
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

    val conversations: StateFlow<Conversations> = viewModelScope.stateOf(list.listChanges()) {
        Conversations(list.state(), list.selectedId(), list.title(), list.query())
    }

    val transcript: StateFlow<Transcript> =
        current
            .flatMapLatest { handles -> transcriptOf(handles.chat) }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000L), Transcript())

    /** The visible draft. Rust echoes every write and coalesces the rest. */
    val draft: StateFlow<String> =
        current
            .flatMapLatest { handles -> handles.draft.text }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000L), "")

    val composerFailure: StateFlow<ErrorSource?> = draftFailure.asStateFlow()

    fun select(id: String?) {
        if (id == conversations.value.selectedId) return
        list.select(id)
        current.value = open(id)
    }

    /** Rust narrows the list; writing this is the whole search implementation. */
    fun search(query: String) {
        if (query == conversations.value.query) return
        list.setQuery(query)
    }

    fun newConversation() {
        opened.remove(null)
        val chat = session.newChat()
        val handles = Handles(chat, chat.composer())
        opened[null] = handles
        follow(handles)
        list.select(null)
        list.setQuery("")
        current.value = handles
    }

    fun rename(chatId: String, title: String) {
        val trimmedTitle = title.trim()
        if (trimmedTitle.isEmpty()) return
        viewModelScope.launch { list.rename(chatId, trimmedTitle) }
    }

    fun delete(chatId: String) {
        viewModelScope.launch {
            if (!list.delete(chatId)) return@launch

            val deleted = opened.remove(chatId)
            if (deleted === current.value) current.value = open(null)
            deleted?.close()
        }
    }

    fun edit(text: String) {
        // `edit` echoes locally and then writes; `viewModelScope` dispatches on
        // the main thread, so the writes reach Rust in typing order.
        val target = current.value.draft
        viewModelScope.launch { target.edit(text) }
    }

    fun send() {
        val state = transcript.value.state
        val text = draft.value
        if (state == null || !state.canSend || text.isBlank()) return
        val chat = current.value.chat
        viewModelScope.launch { chat.send(text) }
    }

    override fun onCleared() {
        opened.values.forEach(Handles::close)
        opened.clear()
        list.close()
    }

    private fun open(id: String?): Handles {
        val handles =
            opened.getOrPut(id) {
                val chat =
                    if (id == null) session.chat() else session.selectChat(id) ?: session.chat()
                Handles(chat, chat.composer()).also { follow(it) }
            }
        val surplus = (opened.size - RESIDENT).coerceAtLeast(0)
        val evicted = opened.entries.filter { it.value !== handles }.take(surplus)
        evicted.forEach { entry ->
            entry.value.close()
            opened.remove(entry.key)
        }
        return handles
    }

    private fun follow(handles: Handles) {
        val composer = handles.composer
        val echoes = projection(composer.composerChanges()) {
            composer.state().text to errorSource(composer.errorKey(), composer.errorArgs())
        }
        handles.followers +=
            viewModelScope.launch { following({ composer.initialize() }, { composer.follow() }) }
        handles.followers +=
            viewModelScope.launch {
                echoes.collect { (text, failure) ->
                    if (handles !== current.value) return@collect
                    handles.draft.absorb(text)
                    draftFailure.value = failure
                }
            }
    }

    private companion object {
        /** Conversations kept mounted at once, the same bound the Windows surface uses. */
        const val RESIDENT = 8
    }

    private fun transcriptOf(chat: ChatHandle) =
        combine(
            rows(chat.chatChanges(), { chat.messagesAfter(it) }, { it.id }),
            projection(chat.chatChanges()) {
                chat.state() to errorSource(chat.errorKey(), chat.errorArgs())
            },
        ) { messages, (state, failure) -> Transcript(state, messages, failure) }
}
