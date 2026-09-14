import ArutBindings
import SwiftUI

struct ConversationView: View {
    let state: ConversationState
    let focusRequest: Int
    @Binding var readingPosition: UInt64?
    @FocusState private var composerFocused: Bool
    @State private var scrollRequest = 0

    var body: some View {
        transcript
            .onChange(of: focusRequest) { composerFocused = true }
            .task {
                composerFocused = true
                await state.run()
            }
    }

    @ViewBuilder
    private var transcript: some View {
        let view = TranscriptView(
            messages: state.messages.rows,
            scrollRequest: scrollRequest,
            readingPosition: $readingPosition
        )
        .overlay {
            if state.isEmpty { emptyState }
        }
        if #available(macOS 26.0, iOS 26.0, *) {
            view.safeAreaBar(edge: .bottom) { composer }
        } else {
            view.safeAreaInset(edge: .bottom) { composer }
        }
    }

    private var emptyState: some View {
        ContentUnavailableView(
            L10n.chatEmptyTitle(),
            systemImage: "bubble.left.and.bubble.right",
            description: Text(L10n.chatEmptyHint())
        )
        .allowsHitTesting(false)
    }

    private var composer: some View {
        ComposerBar(state: state, draft: state.draft, focused: $composerFocused, send: send)
    }

    private func send() {
        Task {
            await state.send()
            scrollRequest += 1
            composerFocused = true
        }
    }
}

private struct ComposerBar: View {
    let state: ConversationState
    @Bindable var draft: Draft
    @FocusState.Binding var focused: Bool
    let send: () -> Void

    var body: some View {
        VStack(alignment: .leading) {
            if let message = state.errorMessage {
                Label(message, systemImage: "exclamationmark.circle.fill")
                    .font(.callout)
                    .foregroundStyle(.red)
                    .accessibilityIdentifier("conversation-error")
            }
            field
                .font(.body)
                .controlSize(.large)
                .padding()
                .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 24))
        }
        .frame(maxWidth: ConversationLayout.maximumWidth)
        .padding()
        .frame(maxWidth: .infinity)
    }

    /// Return sends and Option-Return inserts a line break: that is what a
    /// vertical-axis `TextField` does on macOS on its own, so the surface stays
    /// in SwiftUI and never reaches for the field editor.
    private var hint: String {
        #if os(macOS)
        L10n.composerHintOptionReturn()
        #else
        L10n.composerHintMultiline()
        #endif
    }

    private var field: some View {
        HStack(alignment: .lastTextBaseline) {
            // Keystrokes reach Rust as they are typed; it echoes them back and
            // coalesces, so there is no local queue and no pending-edit counter.
            TextField(L10n.composerPlaceholder(), text: $draft.text, axis: .vertical)
                .textFieldStyle(.plain)
                .lineLimit(1...7)
                .focused($focused)
                .disabled(state.isSending)
                .accessibilityLabel(L10n.labelDraft())
                .accessibilityHint(hint)
                .accessibilityIdentifier("message-composer")
                .help(hint)
                .onSubmit(send)
            sendButton
        }
    }

    private var sendButton: some View {
        Button(action: send) {
            Label(L10n.actionSendMessage(), systemImage: "arrow.up")
                .opacity(state.isSending ? 0 : 1)
                .overlay {
                    if state.isSending { ProgressView().controlSize(.mini) }
                }
        }
        .labelStyle(.iconOnly)
        .buttonStyle(.borderedProminent)
        .buttonBorderShape(.circle)
        .accessibilityLabel(state.isSending ? L10n.chatStatusSending() : L10n.actionSendMessage())
        .disabled(!state.canSend)
        .keyboardShortcut(.return, modifiers: .command)
        .help(L10n.composerHintCommandReturn())
        .accessibilityIdentifier("send-message")
    }
}

enum ConversationLayout {
    // Keep incoming and outgoing messages in one readable column on wide windows.
    static let maximumWidth: CGFloat = 880
}
