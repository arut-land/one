package dev.arut.bindings

import java.util.concurrent.atomic.AtomicBoolean
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/**
 * `Dispatchers.Main` needs a Main dispatcher module (kotlinx-coroutines-android
 * on Android) on the classpath; accessing it without one throws. State is
 * rendered by Compose on Android, so the immediate Main dispatcher avoids an
 * extra hop there, with `Dispatchers.Default` as the fallback on a plain JVM.
 */
private val refreshDispatcher: CoroutineDispatcher =
    try {
        Dispatchers.Main.immediate
    } catch (unavailable: IllegalStateException) {
        Dispatchers.Default
    }

class ObservableState<T>(initial: T) : AutoCloseable {
    private val closed = AtomicBoolean()
    private val mutable = MutableStateFlow(initial)
    private var refresh: Job? = null
    private var subscription: AutoCloseable? = null

    val state: StateFlow<T> = mutable.asStateFlow()

    constructor(
        read: () -> T,
        subscribe: ((ULong) -> Unit) -> AutoCloseable,
    ) : this(read()) {
        observe(read, subscribe)
    }

    fun receive(value: T) {
        mutable.value = value
    }

    fun observe(
        read: () -> T,
        subscribe: ((ULong) -> Unit) -> AutoCloseable,
    ) {
        check(!closed.get()) { "observable state is closed" }
        subscription?.close()
        refresh?.cancel()
        val invalidations = Channel<Unit>(Channel.CONFLATED)
        val scope = CoroutineScope(SupervisorJob() + refreshDispatcher)
        refresh = scope.launch {
            for (ignored in invalidations) receive(read())
        }
        subscription = subscribe { invalidations.trySend(Unit) }
        refresh?.invokeOnCompletion {
            invalidations.close()
            scope.cancel()
        }
    }

    override fun close() {
        if (!closed.compareAndSet(false, true)) return
        subscription?.close()
        refresh?.cancel()
    }
}
