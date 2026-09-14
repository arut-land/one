import ArutBindings
import Foundation
import Observation

/// One conversation's projections, kept current by `run()` for exactly as long
/// as the view that owns it is on screen.
///
/// Every loop over a revision stream belongs to `ArutBindings` (ADR 0021): this
/// holds a `Projection`, a `Rows` cursor and a `Draft`, and adds the one thing a
/// generic helper cannot know -- that this screen shows the chat's error first
/// and the composer's when the chat itself is idle.
@Observable
@MainActor
final class ConversationState {
    let chat: Projection<ChatState>
    let messages: Rows<ChatMessage>
    let draft: Draft
    /// The typed error either scope is holding, already localized.
    let failure: Projection<String?>

    /// A thrown FFI call is a transport failure, not a typed outcome; typed
    /// outcomes arrive through `failure` on the next revision.
    private(set) var transportFailure: String?

    @ObservationIgnored private let handle: ChatHandle
    @ObservationIgnored private let composer: ComposerHandle

    init(chat handle: ChatHandle) {
        let composer = handle.composer()
        self.handle = handle
        self.composer = composer
        self.chat = Projection { handle.state() }
        self.messages = Rows(id: { $0.id }, after: { handle.messagesAfter(afterId: $0) })
        self.draft = Draft(
            composer.state().text,
            replace: { _ = try await composer.replace(text: $0) }
        )
        self.failure = Projection {
            handle.localized(bundle: .module) ?? composer.localized(bundle: .module)
        }
        draft.onFailure = { [weak self] _ in self?.reportTransportFailure() }
    }

    /// The conversation Rust has named, or `nil` while this one is pending.
    var chatId: String? { chat.value.id }
    var errorMessage: String? { failure.value ?? transportFailure }
    var canSend: Bool { chat.value.canSend && !draft.isEmpty }
    var isSending: Bool { chat.value.isSending }
    var isEmpty: Bool { messages.isEmpty }

    /// Runs until the owning view disappears; SwiftUI cancels the task, which
    /// ends every follower and releases the generated subscriptions.
    ///
    /// One subscription per scope: a scope revises its whole projection at once,
    /// so the state, the row cursor and the error are read together.
    func run() async {
        async let transcript: Void = observing(handle.chatChanges()) { [self] in
            chat.refresh()
            messages.refresh()
            failure.refresh()
        }
        async let drafts: Void = observing(composer.composerChanges()) { [self] in
            draft.absorb(remote: composer.state().text)
            failure.refresh()
        }
        async let composing: Void = following(
            initialize: { _ = try await self.composer.initialize() },
            follow: { try await self.composer.follow() },
            onFailure: { [weak self] _ in self?.reportTransportFailure() }
        )
        _ = await (transcript, drafts, composing)
    }

    func send() async {
        guard canSend else { return }
        let text = draft.text
        do {
            _ = try await handle.send(text: text)
            transportFailure = nil
        } catch is CancellationError {
            return
        } catch {
            reportTransportFailure()
        }
    }

    private func reportTransportFailure() {
        transportFailure = L10n.nodeFailureInternal()
    }
}
