import ArutBindings
import SwiftUI
#if os(macOS)
import AppKit
#endif

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
            messages: state.messages,
            scrollRequest: scrollRequest,
            readingPosition: $readingPosition
        )
        if #available(macOS 26.0, iOS 26.0, *) {
            view.safeAreaBar(edge: .bottom) { composer }
        } else {
            view.safeAreaInset(edge: .bottom) { composer }
        }
    }

    private var composer: some View {
        ComposerBar(state: state, focused: $composerFocused, send: send)
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
    @Bindable var state: ConversationState
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

    private var field: some View {
        HStack(alignment: .lastTextBaseline) {
            // Keystrokes reach Rust as they are typed; it echoes them back and
            // coalesces, so there is no local queue and no pending-edit counter.
            TextField(L10n.composerPlaceholder(), text: $state.draft, axis: .vertical)
                .textFieldStyle(.plain)
                .lineLimit(1...7)
                .focused($focused)
                .disabled(state.isSending)
                .accessibilityLabel(L10n.labelDraft())
                .accessibilityHint(L10n.composerHintMultiline())
                .accessibilityIdentifier("message-composer")
                .help(L10n.composerHintMultiline())
                #if os(macOS)
                .onKeyPress(.return, phases: .down) { key in
                    guard key.modifiers.contains(.shift),
                          let editor = NSApp.keyWindow?.firstResponder as? NSTextView,
                          !editor.hasMarkedText() else { return .ignored }
                    // Keep insertion, selection, and undo in the native field editor.
                    editor.insertNewlineIgnoringFieldEditor(nil)
                    return .handled
                }
                #endif
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
