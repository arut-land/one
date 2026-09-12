package dev.arut.surface

import androidx.compose.runtime.Composable
import dev.arut.ffi.ChatError
import dev.arut.ffi.ComposerError
import dev.arut.ffi.NodeFailure
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
    is ComposerError.Node -> describe(error.field0)
    is ComposerError.RevisionConflict -> L10n.composerErrorRevisionConflict(error.current)
    is ComposerError.AuthorityChanged -> L10n.composerErrorAuthorityChanged(error.currentEpoch)
    is ComposerError.SnapshotMissing -> L10n.composerErrorSnapshotMissing()
    is ComposerError.OutcomeMissing -> L10n.composerErrorOutcomeMissing()
    is ComposerError.ScopeMissing -> L10n.composerErrorScopeMissing()
    is ComposerError.ScopeMismatch -> L10n.composerErrorScopeMismatch()
}

@Composable
fun describe(error: ChatError): String = when (error) {
    is ChatError.Node -> describe(error.field0)
    is ChatError.NoConversation -> L10n.chatErrorNoConversation()
    is ChatError.Cancelled -> L10n.chatErrorCancelled()
    is ChatError.Draft -> describe(error.field0)
    is ChatError.ChatIdMissing -> L10n.chatErrorChatIdMissing()
}
