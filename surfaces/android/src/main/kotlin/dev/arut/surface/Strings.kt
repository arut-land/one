package dev.arut.surface

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import dev.arut.ffi.ChatError
import dev.arut.ffi.ComposerError
import dev.arut.ffi.NodeFailure

// The core returns typed outcomes only (ADR 0016) and every sentence lives once
// in product/i18n as Fluent (ADR 0022). This file is only the mapping from a
// variant to its string resource in res/values/strings.xml, which
// `mise run i18n` generates; the lookup and the locale come from Android's own
// resource system, so this surface localizes like any other Android app.
//
// Resource names match `message_key` on the same enum in
// `arut_feature_chat::errors`, with `-` replaced by `_`.

/** The `R.string` id for a failure. */
fun messageResource(failure: NodeFailure): Int = when (failure) {
    NodeFailure.UNREACHABLE -> R.string.node_failure_unreachable
    NodeFailure.TIMED_OUT -> R.string.node_failure_timed_out
    NodeFailure.CANCELLED -> R.string.node_failure_cancelled
    NodeFailure.REFUSED -> R.string.node_failure_refused
    NodeFailure.OVERLOADED -> R.string.node_failure_overloaded
    NodeFailure.REJECTED -> R.string.node_failure_rejected
    NodeFailure.MISSING -> R.string.node_failure_missing
    NodeFailure.CONFLICT -> R.string.node_failure_conflict
    NodeFailure.UNSUPPORTED -> R.string.node_failure_unsupported
    NodeFailure.INTERNAL -> R.string.node_failure_internal
}

@Composable
fun describe(failure: NodeFailure): String = stringResource(messageResource(failure))

@Composable
fun describe(error: ComposerError): String = when (error) {
    is ComposerError.Node -> describe(error.field0)
    is ComposerError.RevisionConflict ->
        stringResource(R.string.composer_error_revision_conflict, error.current)
    is ComposerError.AuthorityChanged ->
        stringResource(R.string.composer_error_authority_changed, error.currentEpoch)
    is ComposerError.SnapshotMissing -> stringResource(R.string.composer_error_snapshot_missing)
    is ComposerError.OutcomeMissing -> stringResource(R.string.composer_error_outcome_missing)
    is ComposerError.ScopeMissing -> stringResource(R.string.composer_error_scope_missing)
    is ComposerError.ScopeMismatch -> stringResource(R.string.composer_error_scope_mismatch)
}

@Composable
fun describe(error: ChatError): String = when (error) {
    is ChatError.Node -> describe(error.field0)
    is ChatError.NoConversation -> stringResource(R.string.chat_error_no_conversation)
    is ChatError.Cancelled -> stringResource(R.string.chat_error_cancelled)
    is ChatError.Draft -> describe(error.field0)
    is ChatError.ChatIdMissing -> stringResource(R.string.chat_error_chat_id_missing)
}
