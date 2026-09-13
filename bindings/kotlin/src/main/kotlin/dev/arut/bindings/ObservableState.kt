package dev.arut.bindings

import java.util.concurrent.atomic.AtomicBoolean
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.flow.conflate
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.onEach

class ObservableState<T>(initial: T) : AutoCloseable {
    private val closed = AtomicBoolean()
    private val mutable = MutableStateFlow(initial)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var refresh: Job? = null

    val state: StateFlow<T> = mutable.asStateFlow()

    constructor(read: () -> T, subscribe: ((ULong) -> Unit) -> AutoCloseable) : this(read()) {
        observe(read, subscribe)
    }

    fun receive(value: T) { mutable.value = value }

    fun observe(read: () -> T, subscribe: ((ULong) -> Unit) -> AutoCloseable) {
        check(!closed.get()) { "observable state is closed" }
        refresh?.cancel()
        refresh = callbackFlow {
            val subscription = subscribe { trySend(Unit) }
            // Read after subscribing even when the source has no initial event.
            trySend(Unit)
            awaitClose { subscription.close() }
        }.conflate().onEach { receive(read()) }.launchIn(scope)
    }

    override fun close() {
        if (closed.compareAndSet(false, true)) scope.cancel()
    }
}
