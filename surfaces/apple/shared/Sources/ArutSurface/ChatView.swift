import ArutBindings
import ArutFfi
import SwiftUI

public struct ChatView: View {
    private let session: ProductSessionHandle
    @State private var selected: ChatHandle
    @StateObject private var conversations: ObservableState<[ChatSummary]>

    // The session is constructed by the composition root and handed to this
    // view (ADR 0007); it is never created here.
    public init(session: ProductSessionHandle) {
        self.session = session
        _selected = State(initialValue: session.chat())
        let list = session.conversations()
        _conversations = StateObject(wrappedValue: ObservableState(read: list.state, subscribe: { callback in
            let subscription = list.listChanges(callback: callback)
            return { subscription.cancel() }
        }))
    }
    public var body: some View {
        NavigationSplitView {
            List {
                Button("New conversation") { selected = session.newChat() }
                ForEach(conversations.state, id: \.id) { summary in
                    Button(summary.title) { if let chat = session.selectChat(id: summary.id) { selected = chat } }
                }
            }
        } detail: {
            ConversationView(chat: selected).id(ObjectIdentifier(selected))
        }
    }
}

private struct ConversationView: View {
    let chat: ChatHandle
    let composer: ComposerHandle
    @StateObject private var transcript: ObservableState<ChatState>
    @StateObject private var draft: ObservableState<ComposerState>
    init(chat: ChatHandle) {
        self.chat = chat
        let composer = chat.composer()
        self.composer = composer
        _transcript = StateObject(wrappedValue: ObservableState(read: chat.state, subscribe: { callback in
            let subscription = chat.chatChanges(callback: callback)
            return { subscription.cancel() }
        }))
        _draft = StateObject(wrappedValue: ObservableState(read: composer.state, subscribe: { callback in
            let subscription = composer.composerChanges(callback: callback)
            return { subscription.cancel() }
        }))
    }
    // The chat's own error takes precedence; a composer-only failure (a
    // background resync, say) still needs to reach the person even when the
    // chat itself is idle.
    private var errorMessage: String? {
        if let error = transcript.state.error { return describe(error) }
        if let error = draft.state.error { return describe(error) }
        return nil
    }
    var body: some View {
        VStack {
            List(transcript.state.messages, id: \.id) { Text($0.text) }
            if let errorMessage {
                Label(errorMessage, systemImage: "exclamationmark.triangle")
                    .foregroundStyle(.red)
                    .font(.footnote)
            }
            HStack {
                TextField("Message Arut", text: Binding(get: { draft.state.text }, set: { text in Task { _ = await composer.replace(text: text) } }))
                Button("Send") { Task { _ = await chat.send(text: composer.state().text) } }
            }
        }.task { _ = await composer.initialize(); await composer.follow() }
    }
}
