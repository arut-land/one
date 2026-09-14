import ArutBindings
import Foundation
import Observation

/// One conversation's projections, kept current by `run()` for exactly as long
/// as the view that owns it is on screen. Observation tracks the properties a
/// `body` actually reads, so there is no adapter class and no snapshot tuple
/// (ADR 0021); the generated streams are consumed with `for await` (ADR 0007).
@Observable
@MainActor
final class ConversationState {
    private(set) var chat: ChatState
    private(set) var messages: [ChatMessage] = []
    /// Already localized: the core hands over a Fluent id and its arguments,
    /// never a sentence (ADR 0016, ADR 0022).
    private(set) var errorMessage: String?

    /// The visible draft. Writing it reaches Rust immediately, which echoes the
    /// text into `ComposerState` and coalesces rapid edits behind one in-flight
    /// write, last one winning, so the surface keeps no queue of its own.
    var draft: String {
        get { draftText }
        set {
            guard newValue != draftText else { return }
            draftText = newValue
            Task { await self.write(newValue) }
        }
    }

    private var draftText: String
    @ObservationIgnored private let handle: ChatHandle
    @ObservationIgnored private let composer: ComposerHandle

    init(chat handle: ChatHandle) {
        let composer = handle.composer()
        self.handle = handle
        self.composer = composer
        self.chat = handle.state()
        self.draftText = composer.state().text
        self.messages = handle.messagesAfter(afterId: 0)
    }

    var canSend: Bool {
        chat.canSend && !draftText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var isSending: Bool { chat.isSending }
    var isEmpty: Bool { messages.isEmpty }

    /// Runs until the owning view disappears; SwiftUI cancels the task, which
    /// ends every loop and releases the generated subscriptions.
    func run() async {
        await perform { _ = try await self.composer.initialize() }
        async let transcript: Void = followChat()
        async let drafts: Void = followComposer()
        async let following: Void = perform { try await self.composer.follow() }
        _ = await (transcript, drafts, following)
    }

    func send() async {
        guard canSend else { return }
        let text = draftText
        await perform { _ = try await self.handle.send(text: text) }
    }

    private func followChat() async {
        for await _ in handle.chatChanges() {
            chat = handle.state()
            messages += handle.messagesAfter(afterId: messages.last?.id ?? 0)
            refreshError()
        }
    }

    private func followComposer() async {
        for await _ in composer.composerChanges() {
            let text = composer.state().text
            if text != draftText { draftText = text }
            refreshError()
        }
    }

    private func write(_ text: String) async {
        await perform { _ = try await self.composer.replace(text: text) }
    }

    private func refreshError() {
        if let key = handle.errorKey() {
            errorMessage = Strings.localized(key, arguments: handle.errorArgs())
        } else if let key = composer.errorKey() {
            errorMessage = Strings.localized(key, arguments: composer.errorArgs())
        } else {
            errorMessage = nil
        }
    }

    /// A thrown FFI call is a transport failure, not a typed outcome; typed
    /// outcomes arrive through `errorKey()` on the next revision.
    private func perform(_ action: () async throws -> Void) async {
        do {
            try await action()
            refreshError()
        } catch is CancellationError {
            return
        } catch {
            errorMessage = Strings.localized("node-failure-internal", arguments: [])
        }
    }
}
