package dev.arut.surface

import androidx.compose.runtime.Composable
import dev.arut.bindings.*
import dev.arut.surface.generated.L10n

// The core returns typed outcomes only (ADR 0016) and every sentence lives once
// in product/i18n as Fluent (ADR 0022). All this file does is choose which
// generated L10n accessor a typed variant means; the resource name, the lookup
// and the locale are in generated/L10n.kt and Android's resource system, so
// this surface localizes the way any other Android app does.

@Composable
fun describe(failure: NodeFailure): String = when (failure) {
    NodeFailure.UNREACHABLE -> L10n.nodeFailureUnreachable()
    NodeFailure.TIMED_OUT -> L10n.nodeFailureTimedOut()
    NodeFailure.CANCELLED -> L10n.nodeFailureCancelled()
    NodeFailure.REFUSED -> L10n.nodeFailureRefused()
    NodeFailure.OVERLOADED -> L10n.nodeFailureOverloaded()
    NodeFailure.REJECTED -> L10n.nodeFailureRejected()
    NodeFailure.MISSING -> L10n.nodeFailureMissing()
    NodeFailure.CONFLICT -> L10n.nodeFailureConflict()
    NodeFailure.UNSUPPORTED -> L10n.nodeFailureUnsupported()
    NodeFailure.INTERNAL -> L10n.nodeFailureInternal()
}

@Composable
fun describe(error: ComposerError): String = when (error) {
    is ComposerErrorNode -> describe(error.field0)
    is ComposerErrorRevisionConflict -> L10n.composerErrorRevisionConflict(error.current.toString())
    is ComposerErrorAuthorityChanged -> L10n.composerErrorAuthorityChanged(error.currentEpoch.toString())
    is ComposerErrorSnapshotMissing -> L10n.composerErrorSnapshotMissing()
    is ComposerErrorOutcomeMissing -> L10n.composerErrorOutcomeMissing()
    is ComposerErrorScopeMissing -> L10n.composerErrorScopeMissing()
    is ComposerErrorScopeMismatch -> L10n.composerErrorScopeMismatch()
}

@Composable
fun describe(error: ChatError): String = when (error) {
    is ChatErrorNode -> describe(error.field0)
    is ChatErrorNoConversation -> L10n.chatErrorNoConversation()
    is ChatErrorCancelled -> L10n.chatErrorCancelled()
    is ChatErrorDraft -> describe(error.field0)
    is ChatErrorChatIdMissing -> L10n.chatErrorChatIdMissing()
}
