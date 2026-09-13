import ArutBindings
import SwiftUI
#if os(macOS)
import AppKit
#endif

struct ConversationView: View {
    let chat: ChatHandle
    let composer: ComposerHandle
    let focusRequest: Int
    @Binding private var readingPosition: UInt64?
    @FocusState private var composerFocused: Bool
    @StateObject private var transcript: ObservableState<(state: ChatState, messages: [ChatMessage])>
    @StateObject private var draft: ObservableState<ComposerState>
    @State private var callFailure: NodeFailure?
    @State private var composerText = ""
    @State private var pendingEdits = 0
    @State private var editTask: Task<Void, Never>?
    @State private var sending = false
    @State private var scrollRequest = 0

    init(chat: ChatHandle, focusRequest: Int, readingPosition: Binding<UInt64?>) {
        self.chat = chat
        self.focusRequest = focusRequest
        _readingPosition = readingPosition
        let composer = chat.composer()
        self.composer = composer
        var messages: [ChatMessage] = []
        _transcript = StateObject(wrappedValue: ObservableState(read: {
            messages += chat.messagesAfter(afterId: messages.last?.id ?? 0)
            return (state: chat.state(), messages: messages)
        }, subscribe: { callback in
            chat.chatChanges(callback: callback).cancel
        }))
        _draft = StateObject(wrappedValue: ObservableState(read: composer.state, subscribe: { callback in
            composer.composerChanges(callback: callback).cancel
        }))
    }

    private var errorMessage: String? {
        if let error = transcript.state.state.error { return describe(error) }
        if let error = draft.state.error { return describe(error) }
        if let callFailure { return describe(callFailure) }
        return nil
    }

    private var canSend: Bool {
        !sending && !composerText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var composerHint: String {
        #if os(macOS)
        if #available(macOS 14.0, *) { return L10n.composerHintMultiline() }
        return L10n.composerHintOptionReturn()
        #else
        return L10n.composerHintMultiline()
        #endif
    }

    private func perform(_ action: () async throws -> Void) async {
        callFailure = nil
        do {
            try await action()
        } catch {
            // SwiftUI cancels the draft follower when its conversation leaves.
            guard !Task.isCancelled else { return }
            callFailure = error is CancellationError ? .cancelled : .internal
        }
    }

    private func send() {
        guard canSend else { return }
        let text = composerText
        let edits = editTask
        sending = true
        Task {
            await edits?.value
            await perform { _ = try await chat.send(text: text) }
            sending = false
            scrollRequest += 1
            composerFocused = true
        }
    }

    private func replaceDraft(_ text: String) {
        // Echo locally before asynchronous acknowledgements arrive. Every edit
        // still reaches Rust in order; send waits for the outstanding writes.
        composerText = text
        pendingEdits += 1
        let previous = editTask
        editTask = Task {
            await previous?.value
            await perform { _ = try await composer.replace(text: text) }
            pendingEdits -= 1
            if pendingEdits == 0 { composerText = composer.state().text }
        }
    }

    var body: some View {
        transcriptWithComposer
            .onReceive(draft.$state) { state in
                if pendingEdits == 0 { composerText = state.text }
            }
            .onChange(of: focusRequest) { _ in composerFocused = true }
            .task {
                composerFocused = true
                await perform {
                    _ = try await composer.initialize()
                    try await composer.follow()
                }
            }
    }

    @ViewBuilder
    private var transcriptWithComposer: some View {
        let transcript = TranscriptView(messages: self.transcript.state.messages,
                                        scrollRequest: scrollRequest, readingPosition: $readingPosition)
        if #available(macOS 26.0, iOS 26.0, *) {
            transcript.safeAreaBar(edge: .bottom) { composerBar }
        } else {
            transcript.safeAreaInset(edge: .bottom) { composerBar }
        }
    }

    private var composerBar: some View {
        VStack(alignment: .leading) {
            if let errorMessage {
                Label(errorMessage, systemImage: "exclamationmark.circle.fill")
                    .font(.callout)
                    .foregroundStyle(.red)
                    .accessibilityIdentifier("conversation-error")
            }
            HStack(alignment: .lastTextBaseline) {
                TextField(L10n.composerPlaceholder(), text: Binding(get: { composerText }, set: replaceDraft), axis: .vertical)
                    .textFieldStyle(.plain)
                    .lineLimit(1...7)
                    .focused($composerFocused)
                    .disabled(sending)
                    .accessibilityLabel(L10n.labelDraft())
                    .accessibilityHint(composerHint)
                    .accessibilityIdentifier("message-composer")
                    .help(composerHint)
                    .modifier(ComposerNewline())
                    .onSubmit(send)
                sendButton
            }
            .font(.body)
            .controlSize(.large)
            .padding()
            .modifier(ComposerGlass())
        }
        .frame(maxWidth: ConversationLayout.maximumWidth)
        .padding()
        .frame(maxWidth: .infinity)
    }

    @ViewBuilder
    private var sendButton: some View {
        let button = Button(action: send) {
            Label(L10n.actionSendMessage(), systemImage: "arrow.up")
                .opacity(sending ? 0 : 1)
                .overlay {
                    if sending { ProgressView().controlSize(.mini) }
                }
        }
        .labelStyle(.iconOnly)
        .accessibilityLabel(sending ? L10n.chatStatusSending() : L10n.actionSendMessage())
        .disabled(!canSend)
        .keyboardShortcut(.return, modifiers: .command)
        .help(L10n.composerHintCommandReturn())
        .accessibilityIdentifier("send-message")
        if #available(macOS 14.0, iOS 17.0, *) {
            button.buttonStyle(.borderedProminent).buttonBorderShape(.circle)
        } else {
            button.buttonStyle(.borderedProminent)
        }
    }
}

enum ConversationLayout {
    // Keep incoming and outgoing messages in one readable column on wide windows.
    static let maximumWidth: CGFloat = 880
}

private struct ComposerNewline: ViewModifier {
    @ViewBuilder
    func body(content: Content) -> some View {
        #if os(macOS)
        if #available(macOS 14.0, *) {
            content.onKeyPress(.return, phases: .down) { key in
                guard key.modifiers.contains(.shift),
                      let editor = NSApp.keyWindow?.firstResponder as? NSTextView,
                      !editor.hasMarkedText() else { return .ignored }
                // Keep insertion, selection, and undo in the native field editor.
                editor.insertNewlineIgnoringFieldEditor(nil)
                return .handled
            }
        } else {
            content
        }
        #else
        content
        #endif
    }
}

private struct ComposerGlass: ViewModifier {
    @ViewBuilder
    func body(content: Content) -> some View {
        if #available(macOS 26.0, iOS 26.0, *) {
            content.glassEffect(.regular, in: .rect(cornerRadius: 24))
        } else {
            content.background(.regularMaterial, in: RoundedRectangle(cornerRadius: 24))
        }
    }
}
