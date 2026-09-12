import ArutFfi
import Foundation

// The core returns typed outcomes only (ADR 0016) and every sentence lives once
// in product/i18n as Fluent (ADR 0022). This file is only the mapping from a
// variant to its key in Resources/Localizable.xcstrings, which `mise run i18n`
// generates; the lookup and the language negotiation are Foundation's, so this
// surface localizes exactly the way any other Apple app does.
//
// Keys match `message_key` on the same enum in `arut_feature_chat::errors`.

private func localized(_ key: String) -> String {
    String(localized: String.LocalizationValue(key), bundle: .module)
}

private func localized(_ key: String, _ argument: CVarArg) -> String {
    String(format: localized(key), argument)
}

func messageKey(_ failure: NodeFailure) -> String {
    switch failure {
    case .unreachable: return "node-failure-unreachable"
    case .timedOut: return "node-failure-timed-out"
    case .cancelled: return "node-failure-cancelled"
    case .refused: return "node-failure-refused"
    case .overloaded: return "node-failure-overloaded"
    case .rejected: return "node-failure-rejected"
    case .missing: return "node-failure-missing"
    case .conflict: return "node-failure-conflict"
    case .unsupported: return "node-failure-unsupported"
    case .internal: return "node-failure-internal"
    }
}

func describe(_ failure: NodeFailure) -> String {
    localized(messageKey(failure))
}

func describe(_ error: ComposerError) -> String {
    switch error {
    case .node(let failure):
        return describe(failure)
    case .revisionConflict(let current):
        return localized("composer-error-revision-conflict", Int64(bitPattern: current))
    case .authorityChanged(let currentEpoch):
        return localized("composer-error-authority-changed", Int64(bitPattern: currentEpoch))
    case .snapshotMissing:
        return localized("composer-error-snapshot-missing")
    case .outcomeMissing:
        return localized("composer-error-outcome-missing")
    case .scopeMissing:
        return localized("composer-error-scope-missing")
    case .scopeMismatch:
        return localized("composer-error-scope-mismatch")
    }
}

func describe(_ error: ChatError) -> String {
    switch error {
    case .node(let failure):
        return describe(failure)
    case .noConversation:
        return localized("chat-error-no-conversation")
    case .cancelled:
        return localized("chat-error-cancelled")
    case .draft(let composerError):
        return describe(composerError)
    case .chatIdMissing:
        return localized("chat-error-chat-id-missing")
    }
}
