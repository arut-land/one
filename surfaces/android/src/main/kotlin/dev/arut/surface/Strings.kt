package dev.arut.surface

import dev.arut.ffi.ChatError
import dev.arut.ffi.ComposerError
import dev.arut.ffi.NodeFailure

// The core returns typed outcomes only (ADR 0016); this surface owns every
// user-facing string. Sentences are written from the person's side of the
// screen: what happened to what they were doing, not what the core did.

fun describe(failure: NodeFailure): String = when (failure) {
    NodeFailure.UNREACHABLE -> "Arut can't reach your node right now."
    NodeFailure.TIMED_OUT -> "Your node is taking too long to answer."
    NodeFailure.CANCELLED -> "That request was cancelled before your node answered."
    NodeFailure.REFUSED -> "Your node refused that request."
    NodeFailure.OVERLOADED -> "Your node is too busy right now. Try again shortly."
    NodeFailure.REJECTED -> "Your node couldn't accept that as it stands."
    NodeFailure.MISSING -> "Your node says that no longer exists."
    NodeFailure.CONFLICT -> "Something else changed first. Try again."
    NodeFailure.UNSUPPORTED -> "Your node doesn't support that yet."
    NodeFailure.INTERNAL -> "Something went wrong on your node."
}

fun describe(error: ComposerError): String = when (error) {
    is ComposerError.Node -> describe(error.field0)
    is ComposerError.RevisionConflict -> "Someone else edited this draft first, so your edit didn't go through."
    is ComposerError.AuthorityChanged -> "This conversation moved to a new authority, so your edit didn't go through. Try again."
    is ComposerError.SnapshotMissing -> "Your node didn't send back the draft, so it may be out of sync."
    is ComposerError.OutcomeMissing -> "Your node didn't say what happened to your edit."
    is ComposerError.ScopeMissing -> "Your node didn't say which draft it meant."
    is ComposerError.ScopeMismatch -> "Your node answered about a different draft."
}

fun describe(error: ChatError): String = when (error) {
    is ChatError.Node -> describe(error.field0)
    is ChatError.NoConversation -> "There's no conversation to send this to yet."
    is ChatError.Cancelled -> "This conversation closed before your message could send."
    is ChatError.Draft -> describe(error.field0)
    is ChatError.ChatIdMissing -> "Your node started a conversation but didn't tell us its name."
}
