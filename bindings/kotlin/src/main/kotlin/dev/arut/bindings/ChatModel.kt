package dev.arut.bindings

import dev.arut.ffi.chatChanges
import dev.arut.ffi.composerChanges
import dev.arut.ffi.createProductSession
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import java.util.concurrent.atomic.AtomicLong

typealias ChatRole = dev.arut.ffi.ChatRole
typealias ChatState = dev.arut.ffi.ChatState
typealias ChatStatus = dev.arut.ffi.ChatStatus
typealias ChatSummary = dev.arut.ffi.ChatSummary
typealias ComposerState = dev.arut.ffi.ComposerState
typealias ComposerStatus = dev.arut.ffi.ComposerStatus

class ChatModel : AutoCloseable {
    private val session = createProductSession("", "local-demo")
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private var core = session.chat()
    private var composer = core.composer()
    private val observable = ObservableState(core::state, core::chatChanges)
    private val composerObservable = ObservableState(composer::state, composer::composerChanges)
    private val inputObservable = ObservableState(composer.state().text)
    private val historyObservable = ObservableState(session.chatSummaries())
    private val selectedChatObservable = ObservableState(core.id())
    private var generation = 0UL
    private var polling: Job? = null
    private val editSequence = AtomicLong()

    val input = inputObservable.state
    val state = observable.state
    val composerState = composerObservable.state
    val history = historyObservable.state
    val selectedChatId = selectedChatObservable.state

    init {
        observeComposer()
    }

    fun setInput(input: String) {
        inputObservable.receive(input)
        val edit = editSequence.incrementAndGet()
        val expectedGeneration = generation
        val target = composer
        scope.launch {
            if (generation != expectedGeneration) return@launch
            val state = target.replace(input)
            if (generation == expectedGeneration && editSequence.get() == edit) {
                composerObservable.receive(state)
            }
        }
    }

    fun send() {
        val message = input.value.trim()
        if (message.isEmpty() || state.value.status == ChatStatus.SENDING) return
        val expectedGeneration = generation
        val targetComposer = composer
        val targetChat = core
        scope.launch {
            targetComposer.replace(message)
            if (generation != expectedGeneration) return@launch
            targetChat.send(message)
            if (generation == expectedGeneration) {
                applyComposer(targetComposer.state())
                historyObservable.receive(session.chatSummaries())
                selectedChatObservable.receive(targetChat.id())
            }
        }
    }

    fun newChat() {
        bind(session.newChat())
    }

    fun selectChat(chatId: String) {
        bind(session.selectChat(chatId))
    }

    fun chatIds(): List<String> = session.chatIds()

    private fun bind(next: dev.arut.ffi.ChatHandle) {
        generation += 1UL
        polling?.cancel()
        composer.close()
        core.close()
        core = next
        composer = next.composer()
        selectedChatObservable.receive(core.id())
        historyObservable.receive(session.chatSummaries())
        observable.observe(core::state, core::chatChanges)
        composerObservable.observe(composer::state, composer::composerChanges)
        applyComposer(composer.state())
        observeComposer()
    }

    private fun observeComposer() {
        val expectedGeneration = generation
        val target = composer
        polling = scope.launch {
            val initialEdit = editSequence.get()
            val initial = target.initialize()
            if (generation != expectedGeneration) return@launch
            if (editSequence.get() == initialEdit) applyComposer(initial)
            while (generation == expectedGeneration) {
                delay(500)
                if (generation != expectedGeneration) return@launch
                val edit = editSequence.get()
                val state = target.syncOnce()
                if (editSequence.get() == edit) applyComposer(state)
            }
        }
    }

    private fun applyComposer(state: ComposerState) {
        composerObservable.receive(state)
        inputObservable.receive(state.text)
    }

    override fun close() {
        generation += 1UL
        scope.cancel()
        inputObservable.close()
        historyObservable.close()
        selectedChatObservable.close()
        composerObservable.close()
        observable.close()
        composer.close()
        core.close()
        session.close()
    }
}
