import ArutFfi

// The core returns typed outcomes only (ADR 0016); this surface owns every
// user-facing string. Sentences are written from the person's side of the
// screen: what happened to what they were doing, not what the core did.

func describe(_ failure: NodeFailure) -> String {
    switch failure {
    case .unreachable:
        return "Arut can't reach your node right now."
    case .timedOut:
        return "Your node is taking too long to answer."
    case .cancelled:
        return "That request was cancelled before your node answered."
    case .refused:
        return "Your node refused that request."
    case .overloaded:
        return "Your node is too busy right now. Try again shortly."
    case .rejected:
        return "Your node couldn't accept that as it stands."
    case .missing:
        return "Your node says that no longer exists."
    case .conflict:
        return "Something else changed first. Try again."
    case .unsupported:
        return "Your node doesn't support that yet."
    case .internal:
        return "Something went wrong on your node."
    }
}

func describe(_ error: ComposerError) -> String {
    switch error {
    case .node(let failure):
        return describe(failure)
    case .revisionConflict:
        return "Someone else edited this draft first, so your edit didn't go through."
    case .authorityChanged:
        return "This conversation moved to a new authority, so your edit didn't go through. Try again."
    case .snapshotMissing:
        return "Your node didn't send back the draft, so it may be out of sync."
    case .outcomeMissing:
        return "Your node didn't say what happened to your edit."
    case .scopeMissing:
        return "Your node didn't say which draft it meant."
    case .scopeMismatch:
        return "Your node answered about a different draft."
    }
}

func describe(_ error: ChatError) -> String {
    switch error {
    case .node(let failure):
        return describe(failure)
    case .noConversation:
        return "There's no conversation to send this to yet."
    case .cancelled:
        return "This conversation closed before your message could send."
    case .draft(let composerError):
        return describe(composerError)
    case .chatIdMissing:
        return "Your node started a conversation but didn't tell us its name."
    }
}
