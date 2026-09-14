import ArutBindings
import Foundation
import Observation

/// What the conversations scope publishes, as one value.
///
/// The list is already narrowed to the query and already carries where each
/// title matched it, and the window title is already chosen; this surface
/// renders them and adds no rule of its own (ADR 0021).
struct ConversationsList: Equatable {
    var summaries: [ChatSummary] = []
    var selectedId: String?
    var title: String?
    var query: String = ""
}

/// Which conversation this session is showing. `pending` is the conversation a
/// new chat starts in, the one Rust has not named yet.
enum Selection: Equatable {
    case pending
    case established(String)

    init(_ id: String?) {
        self = id.map(Selection.established) ?? .pending
    }

    var id: String? {
        if case let .established(id) = self { return id }
        return nil
    }
}

/// The conversation list and which conversation this session is showing. Rust
/// owns the selection, so every surface on one session follows the same one;
/// the sidebar binds straight to it (ADR 0007).
@Observable
final class SessionState {
    let conversations: Projection<ConversationsList>
    private(set) var conversation: ConversationState
    /// Bumped by the menu commands; the views watch it to move focus.
    private(set) var composerFocusRequest = 0
    private(set) var searchFocusRequest = 0

    @ObservationIgnored private let session: ProductSessionHandle
    @ObservationIgnored private let list: ConversationsHandle
    private var opened: Selection = .pending

    init(session: ProductSessionHandle) {
        let list = session.conversations()
        self.session = session
        self.list = list
        self.conversations = Projection {
            ConversationsList(
                summaries: list.state(),
                selectedId: list.selectedId(),
                title: list.title(),
                query: list.query()
            )
        }
        self.conversation = ConversationState(chat: session.chat())
        // A session restored with a conversation already selected opens it
        // rather than the pending one.
        let selected = Selection(conversations.value.selectedId)
        if selected != .pending { adopt(selected) }
    }

    /// `nil` is the session's pending conversation, the one a new chat starts in.
    var selection: String? {
        get { conversations.value.selectedId }
        set { open(Selection(newValue)) }
    }

    /// Rust narrows the list; writing this is the whole search implementation.
    var search: String {
        get { conversations.value.query }
        set {
            guard newValue != conversations.value.query else { return }
            list.setQuery(query: newValue)
            conversations.refresh()
        }
    }

    var matches: [ChatSummary] { conversations.value.summaries }

    /// `nil` means no conversation is named yet, so the surface shows its own
    /// new-conversation label -- the only part of the title that is localized.
    var title: String { conversations.value.title ?? L10n.actionNewConversation() }

    func run() async {
        await conversations.follow(list.listChanges())
    }

    /// Takes a selection another surface on this session made. Driven by the
    /// view, so the projection stays a read and the adoption stays declarative.
    func reconcile() {
        open(Selection(conversations.value.selectedId))
    }

    func newConversation() {
        search = ""
        conversation = ConversationState(chat: session.newChat())
        opened = .pending
        list.select(id: nil)
        conversations.refresh()
        composerFocusRequest += 1
    }

    func focusComposer() { composerFocusRequest += 1 }

    func focusSearch() { searchFocusRequest += 1 }

    func move(_ direction: Int) {
        let rows = matches
        guard !rows.isEmpty else { return }
        let current = rows.firstIndex { $0.id == opened.id }
        let next = current.map { min(max($0 + direction, 0), rows.count - 1) } ?? 0
        open(.established(rows[next].id))
    }

    private func open(_ selection: Selection) {
        guard selection != opened else { return }
        list.select(id: selection.id)
        conversations.refresh()
        adopt(selection)
    }

    private func adopt(_ selection: Selection) {
        guard case let .established(id) = selection else {
            opened = .pending
            conversation = ConversationState(chat: session.chat())
            return
        }
        // A pending conversation becomes an established one the moment its
        // first message is accepted; it is already on screen, so it is not
        // rebuilt and the composer keeps its focus.
        if conversation.chatId == id {
            opened = selection
            return
        }
        guard let chat = session.selectChat(id: id) else { return }
        opened = selection
        conversation = ConversationState(chat: chat)
    }
}
