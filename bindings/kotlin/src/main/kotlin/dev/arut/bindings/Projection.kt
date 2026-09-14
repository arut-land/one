package dev.arut.bindings

import android.content.Context
import dev.arut.ffi.ErrorArg
import java.util.Date
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.emitAll
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.onStart
import kotlinx.coroutines.flow.stateIn

// What every generated handle already offers: a projection to read, a Flow of
// revisions, a typed error named by its Fluent id, and intents. That contract is
// the same for every feature, so the collect-and-read loop is written once, here,
// and a view model holds intents and screen state only (ADR 0007, ADR 0021).

/**
 * A projection read now, and re-read on every revision the handle reports.
 *
 * Cold: each collector subscribes to its own stream and reads once before the
 * first revision, so a change between construction and subscription is not
 * missed.
 */
fun <T> projection(changes: Flow<ULong>, read: () -> T): Flow<T> =
    changes.onStart { emit(0uL) }.map { read() }

/**
 * The same projection as screen state, shared while something is looking.
 *
 * `WhileSubscribed(5_000)` keeps the subscription across a configuration change
 * and drops it when the screen goes away, so no revision is read for a view that
 * is not on screen.
 */
fun <T> CoroutineScope.stateOf(changes: Flow<ULong>, read: () -> T): StateFlow<T> =
    projection(changes, read).stateIn(this, SharingStarted.WhileSubscribed(5_000L), read())

/**
 * Append-only keyed rows, read by cursor, with one rewind rule.
 *
 * Rows arrive through `after(lastId)`, so a revision costs the rows it added
 * rather than the whole list. A source that no longer holds the row the cursor
 * stands on has rebound -- one handle pointed at another subject -- and there is
 * nothing to append to, so the cursor starts over.
 */
fun <Row> rows(
    changes: Flow<ULong>,
    after: (ULong) -> List<Row>,
    id: (Row) -> ULong,
): Flow<List<Row>> = flow {
    val read = cursor(after, id)
    emitAll(projection(changes) { read() })
}

private fun <Row> cursor(after: (ULong) -> List<Row>, id: (Row) -> ULong): () -> List<Row> {
    var held = emptyList<Row>()
    return {
        val mark = held.lastOrNull()?.let(id) ?: 0uL
        held = if (mark == 0uL) {
            after(0uL)
        } else {
            // One row of overlap is the rewind test: the source still holds the
            // row this cursor stands on, or it is showing something else.
            val tail = after(mark - 1uL)
            if (tail.firstOrNull()?.let(id) == mark) held + tail.drop(1) else after(0uL)
        }
        held
    }
}

/**
 * Runs a handle's initialize-then-follow lifetime.
 *
 * Suspends until the work ends or the calling coroutine is cancelled, so the
 * caller owns the lifetime and nothing is launched behind its back.
 */
suspend fun following(initialize: suspend () -> Unit, follow: suspend () -> Unit) {
    initialize()
    follow()
}

/**
 * A text field bound to a projection the core owns.
 *
 * [edit] echoes locally and then writes, which is why it suspends: the caller's
 * scope owns the write, and Rust coalesces rapid edits behind one in-flight
 * request. [absorb] takes the core's echo back only while nothing local is
 * unacknowledged, so the field never reverts a keystroke already typed.
 */
class Draft(initial: String = "", private val replace: suspend (String) -> Unit) {
    private val local = MutableStateFlow(initial)
    private var unacknowledged = 0

    val text: StateFlow<String> = local.asStateFlow()

    suspend fun edit(text: String) {
        if (text == local.value) return
        local.value = text
        unacknowledged++
        try {
            replace(text)
        } finally {
            unacknowledged--
        }
    }

    fun absorb(remote: String) {
        if (unacknowledged > 0 || remote == local.value) return
        local.value = remote
    }
}

/**
 * A handle that names its current error by Fluent id (ADR 0016, ADR 0022).
 *
 * Each argument arrives as the name that selects it and the value it carries,
 * in the order every generated catalog interpolates them. Android's formatter is
 * positional, so the order is what is used; the names are there for anything
 * that formats by name.
 */
interface ErrorSource {
    fun errorKey(): String?

    fun errorArgs(): List<ErrorArg>

    fun errorArgNames(): List<String> = errorArgs().map { it.name }
}

/**
 * A generated handle's error, as a value.
 *
 * Kotlin cannot retrofit an interface onto a generated class, so the key and its
 * arguments are copied into one. A data class rather than an anonymous object,
 * so two reads of an unchanged error compare equal and a `StateFlow` holding one
 * does not emit for every revision.
 */
data class Failure(val key: String, val args: List<ErrorArg>) : ErrorSource {
    override fun errorKey(): String = key

    override fun errorArgs(): List<ErrorArg> = args
}

/** The error a handle is holding, or `null` when it is holding none. */
fun errorSource(key: String?, args: List<ErrorArg>): ErrorSource? =
    key?.let { Failure(it, args) }

/**
 * The sentence for whatever error this source is holding, or `null`.
 *
 * The core hands over a message id and its arguments, never a sentence, so
 * resolution happens here, against the generated id-to-resource map and the
 * app's own resources. A key with no resource comes back as itself, which is
 * visible on screen and impossible to mistake for copy.
 */
fun ErrorSource.localized(context: Context, messages: Map<String, Int>): String? {
    val key = errorKey() ?: return null
    val message = messages[key] ?: return key
    val values = errorArgs().map { it.value }
    return if (values.isEmpty()) {
        context.getString(message)
    } else {
        context.getString(message, *values.toTypedArray())
    }
}

/** The last instant a date can name, in epoch milliseconds: 9999-12-31T23:59:59.999Z. */
private val LATEST_REPRESENTABLE_MILLISECONDS: ULong = 253_402_300_799_999uL

/**
 * An epoch-millisecond stamp as a date, or `null` when there is no such instant.
 *
 * Zero is "not stamped" and anything past the last representable date is a value
 * no clock produced; one bounds rule, so no surface writes the bound.
 */
fun acceptedAt(epochMilliseconds: ULong): Date? =
    if (epochMilliseconds == 0uL || epochMilliseconds > LATEST_REPRESENTABLE_MILLISECONDS) {
        null
    } else {
        Date(epochMilliseconds.toLong())
    }
