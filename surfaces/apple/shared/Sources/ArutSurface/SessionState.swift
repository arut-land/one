import ArutBindings
import Foundation
import Observation

/// The conversation list and which conversation this session is showing. Rust
/// owns the selection, so every surface on one session follows the same one;
/// the sidebar binds straight to it (ADR 0007).
@Observable
@MainActor
final class SessionState {
    private(set) var summaries: [ChatSummary] = []
    private(set) var conversation: ConversationState
    /// Bumped by the menu commands; the views watch it to move focus.
    private(set) var composerFocusRequest = 0
    private(set) var searchFocusRequest = 0
    var search = ""

    @ObservationIgnored private let session: ProductSessionHandle
    @ObservationIgnored private let conversations: ConversationsHandle
    private var selectedId: String?

    init(session: ProductSessionHandle) {
        let conversations = session.conversations()
        self.session = session
        self.conversations = conversations
        self.summaries = conversations.state()
        self.selectedId = conversations.selectedId()
        self.conversation = ConversationState(chat: session.chat())
    }

    /// `nil` is the session's pending conversation, the one a new chat starts in.
    var selection: String? {
        get { selectedId }
        set { open(newValue) }
    }

    var matches: [ChatSummary] {
        let query = search.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return summaries }
        return summaries.filter { $0.title.localizedStandardContains(query) }
    }

    var title: String {
        summaries.first { $0.id == selectedId }?.title ?? L10n.actionNewConversation()
    }

    func run() async {
        for await _ in conversations.listChanges() {
            summaries = conversations.state()
            let id = conversations.selectedId()
            if id != selectedId { adopt(id) }
        }
    }

    func newConversation() {
        search = ""
        conversation = ConversationState(chat: session.newChat())
        conversations.select(id: nil)
        selectedId = nil
        composerFocusRequest += 1
    }

    func focusComposer() { composerFocusRequest += 1 }

    func focusSearch() { searchFocusRequest += 1 }

    func move(_ direction: Int) {
        let rows = matches
        guard !rows.isEmpty else { return }
        let current = rows.firstIndex { $0.id == selectedId }
        let next = current.map { min(max($0 + direction, 0), rows.count - 1) } ?? 0
        open(rows[next].id)
    }

    private func open(_ id: String?) {
        guard id != selectedId else { return }
        conversations.select(id: id)
        adopt(id)
    }

    private func adopt(_ id: String?) {
        guard let id else {
            selectedId = nil
            conversation = ConversationState(chat: session.chat())
            return
        }
        guard let chat = session.selectChat(id: id) else { return }
        selectedId = id
        conversation = ConversationState(chat: chat)
    }
}
