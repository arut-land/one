import ArutBindings
import SwiftUI

public struct ChatView: View {
    private let session: ProductSessionHandle
    @State private var selected: ChatHandle
    @State private var selection: String? = "pending"
    @State private var search = ""
    @State private var focusRequest = 0
    @State private var readingPositions: [String: UInt64] = [:]
    @FocusState private var searchFocused: Bool
    @StateObject private var conversations: ObservableState<[ChatSummary]>

    public init(session: ProductSessionHandle) {
        self.session = session
        _selected = State(initialValue: session.chat())
        let list = session.conversations()
        _conversations = StateObject(wrappedValue: ObservableState(read: list.state, subscribe: { callback in
            list.listChanges(callback: callback).cancel
        }))
    }

    private var matches: [ChatSummary] {
        let query = search.trimmingCharacters(in: .whitespacesAndNewlines)
        return conversations.state.filter { query.isEmpty || $0.title.localizedStandardContains(query) }
    }

    private func newConversation() {
        search = ""
        selected = session.newChat()
        selection = "pending"
        focusRequest += 1
    }

    private func moveConversation(_ direction: Int) {
        guard !matches.isEmpty else { return }
        let current = matches.firstIndex { $0.id == selection }
        let next = current.map { min(max($0 + direction, 0), matches.count - 1) } ?? 0
        selection = matches[next].id
    }

    private var readingPosition: Binding<UInt64?> {
        let id = selected.state().id ?? "pending"
        return Binding(get: { readingPositions[id] }, set: { readingPositions[id] = $0 })
    }

    public var body: some View {
        NavigationSplitView {
            List(selection: $selection) {
                Section {
                    Label(L10n.actionNewConversation(), systemImage: "square.and.pencil")
                        .tag("pending")
                }
                Section(L10n.labelRecent()) {
                    if matches.isEmpty {
                        Text(search.isEmpty ? L10n.chatHistoryEmpty() : L10n.conversationSearchEmpty())
                            .foregroundStyle(.secondary)
                    }
                    ForEach(matches, id: \.id) { summary in
                        Label {
                            Text(summary.title)
                                .lineLimit(2)
                        } icon: {
                            Image(systemName: "bubble.left")
                                .foregroundStyle(.secondary)
                        }
                        .tag(summary.id)
                        .help(summary.title)
                    }
                }
            }
            .listStyle(.sidebar)
            .searchable(text: $search, placement: .sidebar, prompt: L10n.conversationSearchPlaceholder())
            .modifier(ConversationSearchFocus(focus: $searchFocused))
            .navigationTitle(L10n.appName())
            .navigationSplitViewColumnWidth(min: 210, ideal: 250, max: 340)
            .safeAreaInset(edge: .bottom) {
                Label(L10n.chatLocalSession(), systemImage: "desktopcomputer")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding()
            }
        } detail: {
            ConversationView(chat: selected, focusRequest: focusRequest, readingPosition: readingPosition)
                .id(ObjectIdentifier(selected))
                .navigationTitle(conversations.state.first(where: { $0.id == selected.state().id })?.title ?? L10n.actionNewConversation())
                .toolbar {
                    ToolbarItem(placement: .primaryAction) {
                        Button(action: newConversation) {
                            Label(L10n.actionNewConversation(), systemImage: "square.and.pencil")
                        }
                        .help(L10n.actionNewConversationShortcut(shortcut: "⌘N"))
                    }
                }
        }
        .navigationSplitViewStyle(.balanced)
        .onChange(of: selection) { id in
            if id == "pending", selected.state().id != nil {
                newConversation()
            } else if let id, id != selected.state().id, let chat = session.selectChat(id: id) {
                selected = chat
            }
        }
        .onChange(of: conversations.state.map(\.id)) { _ in
            selection = selected.state().id ?? "pending"
        }
        #if os(macOS)
        .focusedSceneValue(\.newConversation, newConversation)
        .focusedSceneValue(\.focusComposer, { focusRequest += 1 })
        .focusedSceneValue(\.searchConversations, { searchFocused = true })
        .focusedSceneValue(\.moveConversation, moveConversation)
        #endif
    }
}

private struct ConversationSearchFocus: ViewModifier {
    var focus: FocusState<Bool>.Binding

    @ViewBuilder
    func body(content: Content) -> some View {
        if #available(macOS 15.0, iOS 18.0, *) {
            content.searchFocused(focus)
        } else {
            content
        }
    }
}
