package dev.arut.surface

import androidx.lifecycle.ViewModel
import dev.arut.ffi.ProductSessionHandle
import dev.arut.ffi.createProductSession

/**
 * Owns the core session for this process. A ViewModel survives configuration
 * changes (ADR 0011), unlike state held in `remember {}` on the Activity, so
 * the composition root constructs the session here once and hands it to
 * views (ADR 0007). When the foreground service lands, this becomes a bind and
 * `onCleared` an unbind; the seam is deliberately the same shape.
 */
class SessionViewModel : ViewModel() {
    val session: ProductSessionHandle = createProductSession("local-demo")

    override fun onCleared() {
        session.close()
    }
}
