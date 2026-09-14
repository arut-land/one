import ArutBindings
import SwiftUI

public struct ChatView: View {
    @State private var state: SessionState
    @State private var readingPositions: [String: UInt64] = [:]
    /// Window restoration: the pane the window was left showing (macOS rule 2.5).
    @SceneStorage("conversation-selection") private var restoredSelection = ""

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
        .task {
            restore()
            await state.run()
        }
        // Rust owns the selection, so another surface on this session can move
        // it; adopting it is a reaction to the projection, not a second loop.
        .onChange(of: state.selection) {
            state.reconcile()
            restoredSelection = state.selection ?? ""
        }
        #if os(macOS)
        .focusedSceneValue(\.conversations, state)
        #endif
    }

    private func restore() {
        guard state.selection == nil,
              state.matches.contains(where: { $0.id == restoredSelection })
        else { return }
        state.selection = restoredSelection
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
            .safeAreaInset(edge: .bottom) { bottomBar }
    }

    /// The new-conversation action belongs beside the list, not in it: a button
    /// inside a `List(selection:)` takes part in selection and arrow navigation.
    private var bottomBar: some View {
        HStack {
            Button(action: state.newConversation) {
                Label(L10n.actionNewConversation(), systemImage: "square.and.pencil")
            }
            .buttonStyle(.borderless)
            .help(L10n.actionNewConversationShortcut(shortcut: "⌘N"))
            Spacer()
            Label(L10n.chatLocalSession(), systemImage: "desktopcomputer")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding()
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
            Section(L10n.labelRecent()) {
                ForEach(state.matches, id: \.id) { summary in
                    ConversationRow(summary: summary)
                }
            }
        }
        // A `ContentUnavailableView` fills a view; as a list row it would take
        // row insets and sit under a section header.
        .overlay {
            if state.matches.isEmpty { emptyState }
        }
    }

    private var emptyState: some View {
        ContentUnavailableView(
            state.search.isEmpty ? L10n.chatHistoryEmpty() : L10n.conversationSearchEmpty(),
            systemImage: state.search.isEmpty ? "bubble.left" : "magnifyingglass"
        )
    }
}

private struct ConversationRow: View {
    let summary: ChatSummary

    var body: some View {
        Label {
            HStack {
                Text(summary.title).lineLimit(2)
                Spacer(minLength: 8)
                // Rust decides what is unread; the dot only draws it.
                if summary.unread {
                    Circle()
                        .fill(Color.accentColor)
                        .frame(width: 8, height: 8)
                        .accessibilityHidden(true)
                }
            }
        } icon: {
            Image(systemName: "bubble.left").foregroundStyle(.secondary)
        }
        .accessibilityValue(summary.unread ? L10n.labelUnreadMessages() : "")
        .tag(summary.id)
        .help(summary.title)
    }
}

#Preview {
    ChatView(session: createProductSession(pendingScopeId: "preview"))
        .frame(width: 900, height: 600)
}
