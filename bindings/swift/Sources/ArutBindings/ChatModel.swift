import ArutFfi
import Combine
import Foundation

@MainActor
public final class ChatModel: ObservableState<ChatState> {
    private let session: ProductSessionHandle
    private var core: ChatHandle
    private var composer: ComposerHandle
    private var cancelComposer: (() -> Void)?
    private var polling: Task<Void, Never>?
    private var epoch: UInt64 = 0
    private var applyingDraft = false
    private var pendingEdits = 0
    @Published public var draft = "" {
        didSet {
            guard !applyingDraft, draft != oldValue else { return }
            let text = draft
            let expectedEpoch = epoch
            let target = composer
            pendingEdits += 1
            Task {
                defer { pendingEdits -= 1 }
                guard expectedEpoch == epoch else { return }
                if let state = try? await target.replace(text: text), pendingEdits == 1 {
                    apply(state)
                }
            }
        }
    }

    public init() {
        let session = createProductSession(backendUrl: "", pendingScopeId: "local-demo")
        let core = session.chat()
        self.session = session
        self.core = core
        self.composer = core.composer()
        super.init(
            read: core.state,
            subscribe: { invalidate in
                let changes = core.chatChanges(callback: invalidate)
                return changes.cancel
            }
        )
        bindComposer()
    }

    public func send() {
        let message = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !message.isEmpty, state.status != .sending else { return }
        let expectedEpoch = epoch
        let targetComposer = composer
        let targetChat = core
        Task {
            _ = try? await targetComposer.replace(text: message)
            guard expectedEpoch == epoch else { return }
            _ = try? await targetChat.send(text: message)
            guard expectedEpoch == epoch else { return }
            apply(targetComposer.state())
            receive(targetChat.state())
        }
    }

    public func newChat() {
        bind(session.newChat())
    }

    public var chatIDs: [String] {
        session.chatIds()
    }

    public var history: [ChatSummary] {
        session.chatSummaries()
    }

    public var selectedChatID: String {
        core.id()
    }

    public func selectChat(_ chatID: String) {
        bind(session.selectChat(chatId: chatID))
    }

    private func bind(_ next: ChatHandle) {
        epoch &+= 1
        polling?.cancel()
        cancelComposer?()
        core = next
        composer = next.composer()
        receive(next.state())
        observe(
            read: next.state,
            subscribe: { invalidate in
                let changes = next.chatChanges(callback: invalidate)
                return changes.cancel
            }
        )
        bindComposer()
    }

    private func bindComposer() {
        let expectedEpoch = epoch
        let target = composer
        apply(target.state())
        cancelComposer = target.composerChanges { [weak self] _ in
            Task { @MainActor [weak self] in
                guard let self, self.epoch == expectedEpoch else { return }
                self.apply(target.state())
            }
        }.cancel
        polling = Task { [weak self] in
            guard let self else { return }
            _ = try? await target.initialize()
            while !Task.isCancelled, self.epoch == expectedEpoch {
                try? await Task.sleep(nanoseconds: 500_000_000)
                guard !Task.isCancelled, self.epoch == expectedEpoch else { return }
                if let state = try? await target.syncOnce() {
                    self.apply(state)
                }
            }
        }
    }

    private func apply(_ state: ComposerState) {
        guard pendingEdits == 0 else { return }
        guard draft != state.text else { return }
        applyingDraft = true
        draft = state.text
        applyingDraft = false
    }

    deinit {
        polling?.cancel()
        cancelComposer?()
        stopObserving()
    }
}
