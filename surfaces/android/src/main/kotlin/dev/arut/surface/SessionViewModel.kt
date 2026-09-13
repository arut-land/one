package dev.arut.surface

import androidx.lifecycle.ViewModel
import dev.arut.bindings.ProductSessionHandle
import dev.arut.bindings.createProductSession

/**
 * Owns the core session for this process. A ViewModel survives configuration
 * changes (ADR 0011), unlike state held in `remember {}` on the Activity, so
 * the composition root constructs the session here once and hands it to
 * views (ADR 0007).
 */
class SessionViewModel : ViewModel() {
    val session: ProductSessionHandle = createProductSession("local-demo")

    override fun onCleared() {
        session.close()
    }
}
