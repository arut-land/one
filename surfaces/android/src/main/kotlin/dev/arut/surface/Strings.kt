package dev.arut.surface

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource

/**
 * The core names an error by its Fluent message id and supplies the arguments
 * that id takes; no sentence crosses the boundary and the core still learns no
 * locale (ADR 0016, ADR 0022). `R` is Android's own compile-checked accessor,
 * so this table is the only place a key is written by hand -- and
 * `product/i18n/tests/error_keys.rs` fails if a key here has no message.
 */
@Composable
fun localized(key: String, arguments: List<String>): String {
    val message = MESSAGES[key] ?: return key
    return if (arguments.isEmpty()) {
        stringResource(message)
    } else {
        stringResource(message, *arguments.toTypedArray())
    }
}

private val MESSAGES: Map<String, Int> =
    mapOf(
        "node-failure-unreachable" to R.string.node_failure_unreachable,
        "node-failure-timed-out" to R.string.node_failure_timed_out,
        "node-failure-cancelled" to R.string.node_failure_cancelled,
        "node-failure-refused" to R.string.node_failure_refused,
        "node-failure-overloaded" to R.string.node_failure_overloaded,
        "node-failure-rejected" to R.string.node_failure_rejected,
        "node-failure-missing" to R.string.node_failure_missing,
        "node-failure-conflict" to R.string.node_failure_conflict,
        "node-failure-unsupported" to R.string.node_failure_unsupported,
        "node-failure-internal" to R.string.node_failure_internal,
        "composer-error-revision-conflict" to R.string.composer_error_revision_conflict,
        "composer-error-authority-changed" to R.string.composer_error_authority_changed,
        "composer-error-snapshot-missing" to R.string.composer_error_snapshot_missing,
        "composer-error-outcome-missing" to R.string.composer_error_outcome_missing,
        "composer-error-scope-missing" to R.string.composer_error_scope_missing,
        "composer-error-scope-mismatch" to R.string.composer_error_scope_mismatch,
        "chat-error-cancelled" to R.string.chat_error_cancelled,
        "chat-error-chat-id-missing" to R.string.chat_error_chat_id_missing,
    )
