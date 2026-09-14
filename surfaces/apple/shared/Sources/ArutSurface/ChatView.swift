import ArutBindings
import SwiftUI

public struct ChatView: View {
    @State private var state: SessionState
    @State private var readingPositions: [String: UInt64] = [:]

    @MainActor
    public init(session: ProductSessionHandle) {
        _state = State(initialValue: SessionState(session: session))
    }

    public var body: some View {
        NavigationSplitView {
            ConversationSidebar(state: state)
        } detail: {
            ConversationView(
                state: state.conversation,
                focusRequest: state.composerFocusRequest,
                readingPosition: readingPosition
            )
            .id(ObjectIdentifier(state.conversation))
            .navigationTitle(state.title)
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Button(action: state.newConversation) {
                        Label(L10n.actionNewConversation(), systemImage: "square.and.pencil")
                    }
                    .help(L10n.actionNewConversationShortcut(shortcut: "⌘N"))
                }
            }
        }
        .navigationSplitViewStyle(.balanced)
        .task { await state.run() }
        #if os(macOS)
        .focusedSceneValue(\.conversations, state)
        #endif
    }

    /// Reading positions are presentation state, kept per conversation for as
    /// long as the window lives; Rust owns no scroll offset.
    private var readingPosition: Binding<UInt64?> {
        let key = state.selection ?? ""
        return Binding(
            get: { readingPositions[key] },
            set: { readingPositions[key] = $0 }
        )
    }
}

private struct ConversationSidebar: View {
    @Bindable var state: SessionState
    @FocusState private var searchFocused: Bool

    var body: some View {
        searchable
            .navigationTitle(L10n.appName())
            .navigationSplitViewColumnWidth(min: 210, ideal: 250, max: 340)
            .onChange(of: state.searchFocusRequest) { searchFocused = true }
            .safeAreaInset(edge: .bottom) {
                Label(L10n.chatLocalSession(), systemImage: "desktopcomputer")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding()
            }
    }

    // `searchFocused` reaches the native search field only on macOS 15/iOS 18.
    @ViewBuilder
    private var searchable: some View {
        let list = conversations
            .listStyle(.sidebar)
            .searchable(
                text: $state.search,
                placement: .sidebar,
                prompt: L10n.conversationSearchPlaceholder()
            )
        if #available(macOS 15.0, iOS 18.0, *) {
            list.searchFocused($searchFocused)
        } else {
            list
        }
    }

    private var conversations: some View {
        List(selection: $state.selection) {
            Section {
                Button(action: state.newConversation) {
                    Label(L10n.actionNewConversation(), systemImage: "square.and.pencil")
                }
                .buttonStyle(.plain)
            }
            Section(L10n.labelRecent()) {
                if state.matches.isEmpty {
                    ContentUnavailableView(
                        state.search.isEmpty
                            ? L10n.chatHistoryEmpty() : L10n.conversationSearchEmpty(),
                        systemImage: state.search.isEmpty ? "bubble.left" : "magnifyingglass"
                    )
                }
                ForEach(state.matches, id: \.id) { summary in
                    Label {
                        Text(summary.title).lineLimit(2)
                    } icon: {
                        Image(systemName: "bubble.left").foregroundStyle(.secondary)
                    }
                    .tag(summary.id)
                    .help(summary.title)
                }
            }
        }
    }
}

#Preview {
    ChatView(session: createProductSession(pendingScopeId: "preview"))
        .frame(width: 900, height: 600)
}
