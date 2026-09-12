import ArutBindings
import SwiftUI

public struct ChatView: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @ObservedObject private var model: ChatModel
    @FocusState private var composerFocused: Bool

    public init(model: ChatModel) {
        self.model = model
    }

    public var body: some View {
        NavigationSplitView {
            sidebar
        } detail: {
            conversation
        }
        .navigationSplitViewStyle(.balanced)
    }

    private var selection: Binding<String?> {
        Binding(
            get: { model.selectedChatID },
            set: { chatID in
                guard let chatID, chatID != model.selectedChatID else { return }
                model.selectChat(chatID)
            }
        )
    }

    private var sidebar: some View {
        List(selection: selection) {
            Section {
                ForEach(model.history, id: \.id) { chat in
                    NavigationLink(value: chat.id) {
                        Label {
                            Text(chat.title)
                                .lineLimit(2)
                        } icon: {
                            Image(systemName: "message")
                                .foregroundStyle(.secondary)
                        }
                        .padding(.vertical, 3)
                    }
                    .tag(chat.id)
                    .contextMenu {
                        Button("Open") {
                            model.selectChat(chat.id)
                        }
                        Button("New chat", action: startNewChat)
                    }
                }
            } header: {
                Text("Recent")
            }
        }
        .listStyle(.sidebar)
        .navigationSplitViewColumnWidth(min: 220, ideal: 280, max: 360)
        .navigationTitle("Arut")
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button(action: startNewChat) {
                    Label("New chat", systemImage: "square.and.pencil")
                }
                .help("New chat")
            }
        }
    }

    private var conversation: some View {
        ZStack {
            Color.primary.opacity(0.025)
                .ignoresSafeArea()

            messages
        }
        .navigationTitle(selectedTitle)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button(action: startNewChat) {
                    Label("New chat", systemImage: "square.and.pencil")
                }
                .help("New chat")
                .keyboardShortcut("n", modifiers: .command)
            }
#if os(macOS)
            ToolbarItem(placement: .automatic) {
                Button {
                    composerFocused = true
                } label: {
                    Label("Focus composer", systemImage: "text.cursor")
                }
                .help("Focus composer (Command-L)")
                .keyboardShortcut("l", modifiers: .command)
            }
#endif
        }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            composer
        }
        .frame(minWidth: 340, minHeight: 440)
    }

    private var selectedTitle: String {
        model.history.first(where: { $0.id == model.selectedChatID })?.title ?? "New conversation"
    }

    private var messages: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 18) {
                    if model.state.messages.isEmpty {
                        emptyState
                    }

                    ForEach(model.state.messages, id: \.id) { message in
                        let isUser = message.role == .user

                        HStack(alignment: .bottom, spacing: 10) {
                            if isUser { Spacer(minLength: 48) }

                            VStack(alignment: isUser ? .trailing : .leading, spacing: 5) {
                                Text(isUser ? "You" : "Arut")
                                    .font(.caption.weight(.medium))
                                    .foregroundStyle(.secondary)

                                Text(message.text)
                                    .textSelection(.enabled)
                                    .padding(.horizontal, 14)
                                    .padding(.vertical, 10)
                                    .foregroundStyle(isUser ? Color.white : Color.primary)
                                    .background(isUser ? Color.accentColor : Color.primary.opacity(0.07))
                                    .clipShape(RoundedRectangle(cornerRadius: 17, style: .continuous))
                            }

                            if !isUser { Spacer(minLength: 48) }
                        }
                        .frame(maxWidth: .infinity)
                        .id(message.id)
                    }
                }
                .frame(maxWidth: 760)
                .frame(maxWidth: .infinity)
                .padding(.horizontal, 20)
                .padding(.vertical, 24)
            }
            .onChange(of: model.state.messages.count) { _ in
                guard let last = model.state.messages.last else { return }
                withAnimation(reduceMotion ? nil : .easeOut(duration: 0.25)) {
                    proxy.scrollTo(last.id, anchor: .bottom)
                }
            }
        }
    }

    private var emptyState: some View {
        VStack(spacing: 12) {
            Image(systemName: "bubble.left.and.bubble.right")
                .font(.system(size: 34, weight: .light))
                .foregroundStyle(.tint)
                .padding(.bottom, 4)
            Text("Start a conversation")
                .font(.title3.weight(.semibold))
            Text("Ask a question or share what you are working on.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity, minHeight: 320)
        .padding()
    }

    private var composer: some View {
        VStack(spacing: 0) {
            if !model.state.error.isEmpty {
                Label(model.state.error, systemImage: "exclamationmark.circle.fill")
                    .font(.caption)
                    .foregroundStyle(.red)
                    .frame(maxWidth: 760, alignment: .leading)
                    .padding(.horizontal, 20)
                    .padding(.top, 10)
            }

            HStack(alignment: .bottom, spacing: 10) {
                TextField("Message Arut", text: $model.draft, axis: .vertical)
                    .textFieldStyle(.plain)
                    .focused($composerFocused)
                    .lineLimit(1...5)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 10)
                    .background(.regularMaterial)
                    .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                    .overlay {
                        RoundedRectangle(cornerRadius: 16, style: .continuous)
                            .strokeBorder(Color.primary.opacity(0.12))
                    }
                    .onSubmit(model.send)

                Button(action: model.send) {
                    Image(systemName: model.state.status == .sending ? "ellipsis" : "arrow.up")
                        .font(.system(size: 15, weight: .bold))
                        .frame(width: 20, height: 20)
                }
                .buttonStyle(.borderedProminent)
                .buttonBorderShape(.circle)
                .controlSize(.large)
                .disabled(!canSend)
                .accessibilityLabel(model.state.status == .sending ? "Sending" : "Send message")
            }
            .frame(maxWidth: 760)
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
        }
        .frame(maxWidth: .infinity)
        .background(.ultraThinMaterial)
        .overlay(alignment: .top) {
            Divider()
        }
    }

    private var canSend: Bool {
        model.state.status != .sending
            && !model.draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private func startNewChat() {
        model.newChat()
        composerFocused = true
    }
}
