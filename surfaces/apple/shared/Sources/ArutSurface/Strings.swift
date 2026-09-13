import ArutBindings

// The core returns typed outcomes only (ADR 0016) and every sentence lives once
// in product/i18n as Fluent (ADR 0022). All this file does is choose which
// generated L10n accessor a typed variant means; the key, the lookup and the
// language negotiation are in Generated/L10n.swift and Foundation, so this
// surface localizes exactly the way any other Apple app does.

func describe(_ failure: NodeFailure) -> String {
    switch failure {
    case .unreachable: return L10n.nodeFailureUnreachable()
    case .timedOut: return L10n.nodeFailureTimedOut()
    case .cancelled: return L10n.nodeFailureCancelled()
    case .refused: return L10n.nodeFailureRefused()
    case .overloaded: return L10n.nodeFailureOverloaded()
    case .rejected: return L10n.nodeFailureRejected()
    case .missing: return L10n.nodeFailureMissing()
    case .conflict: return L10n.nodeFailureConflict()
    case .unsupported: return L10n.nodeFailureUnsupported()
    case .internal: return L10n.nodeFailureInternal()
    }
}

func describe(_ error: ComposerError) -> String {
    switch error {
    case .node(let failure):
        return describe(failure)
    case .revisionConflict(let current):
        return L10n.composerErrorRevisionConflict(current: String(current))
    case .authorityChanged(let currentEpoch):
        return L10n.composerErrorAuthorityChanged(currentEpoch: String(currentEpoch))
    case .snapshotMissing: return L10n.composerErrorSnapshotMissing()
    case .outcomeMissing: return L10n.composerErrorOutcomeMissing()
    case .scopeMissing: return L10n.composerErrorScopeMissing()
    case .scopeMismatch: return L10n.composerErrorScopeMismatch()
    }
}

func describe(_ error: ChatError) -> String {
    switch error {
    case .node(let failure): return describe(failure)
    case .noConversation: return L10n.chatErrorNoConversation()
    case .cancelled: return L10n.chatErrorCancelled()
    case .draft(let composerError): return describe(composerError)
    case .chatIdMissing: return L10n.chatErrorChatIdMissing()
    }
}
