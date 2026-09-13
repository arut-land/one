import dev.arut.bindings.ObservableState
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.setMain

@OptIn(ExperimentalCoroutinesApi::class)
class ObservationTest {
    @Test fun burstAndLifecycle() = runTest {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        try {
            var value = 0
            var reads = 0
            var cancellations = 0
            var notify: (ULong) -> Unit = {}
            val observer = ObservableState(read = { reads++; value }, subscribe = {
                notify = it
                value = 1 // No initial notification; setup must still refresh.
                AutoCloseable { cancellations++ }
            })
            runCurrent()
            assertEquals(1, observer.state.value)
            val initialReads = reads
            repeat(10_000) { value++; notify(it.toULong()) }
            runCurrent()
            assertEquals(10_001, observer.state.value)
            // A suspended collector can own one signal while one stays conflated.
            assertTrue(reads - initialReads in 1..2)
            val stale = notify
            observer.observe({ 42 }) { notify = it; AutoCloseable { cancellations++ } }
            runCurrent()
            assertEquals(1, cancellations)
            stale(99uL)
            runCurrent()
            assertEquals(42, observer.state.value)
            observer.close()
            observer.close()
            runCurrent()
            assertEquals(2, cancellations)
            notify(100uL)
            runCurrent()
            assertEquals(42, observer.state.value)
        } finally { Dispatchers.resetMain() }
    }
}
