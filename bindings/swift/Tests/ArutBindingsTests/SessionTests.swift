import ArutBindings
import Testing

@MainActor
struct SessionTests {
    @Test(.timeLimit(.minutes(1)))
    func sendUpdatesObservedTranscriptAndConversationList() async throws {
        let session = createProductSession(pendingScopeId: "swift-test")
        let initialized = try await session.initialize()
        #expect(initialized)
        let chat = session.chat()
        let composer = chat.composer()
        let list = session.conversations()
        // Subscribing before the send is what makes the revision observable;
        // the generated stream buffers, so consuming it afterwards is enough.
        let changes = chat.chatChanges()

        _ = try await composer.replace(text: "Hello from Swift")
        #expect(composer.state().text == "Hello from Swift")
        let result = try await chat.send(text: composer.state().text)
        #expect(result.error == nil)

        var transcript: [ChatMessage] = []
        for await _ in changes {
            transcript = chat.messagesAfter(afterId: 0)
            if transcript.count >= 2 { break }
        }
        #expect(transcript.count == 2)
        #expect(transcript.first?.text == "Hello from Swift")
        #expect(list.state().count == 1)
        #expect(composer.state().text == "")
        let id = try #require(result.id)
        #expect(list.selectedId() == nil || list.selectedId() == id)
        #expect(session.selectChat(id: id)?.messagesAfter(afterId: 0).count == 2)
    }

    @Test
    func establishedAndPendingConversationsKeepIndependentDrafts() async throws {
        let session = createProductSession(pendingScopeId: "swift-drafts")
        let chat = session.chat()
        _ = try await chat.send(text: "Start the first conversation")
        let first = chat.composer()
        _ = try await first.replace(text: "First line\n\nThird line with café and 👋")
        let second = session.newChat().composer()
        _ = try await second.replace(text: "Second draft")
        #expect(first.state().text == "First line\n\nThird line with café and 👋")
        #expect(second.state().text == "Second draft")
    }

    @Test
    func multilineMessagePreservesPlainTextAndClearsDraft() async throws {
        let session = createProductSession(pendingScopeId: "swift-multiline")
        let chat = session.chat()
        let composer = chat.composer()
        let text = "First line\n\n• A plain text bullet\n    Indented text with café and 👋\n" +
            (1...12).map { "Line \($0)" }.joined(separator: "\n")
        _ = try await composer.replace(text: text)
        #expect(composer.state().text == text)
        let sent = try await chat.send(text: composer.state().text)
        #expect(sent.error == nil)
        #expect(chat.messagesAfter(afterId: 0).first?.text == text)
        #expect(composer.state().text.isEmpty)
    }

    @Test
    func selectionIsSharedThroughTheConversationsScope() async throws {
        let session = createProductSession(pendingScopeId: "swift-selection")
        let chat = session.chat()
        let result = try await chat.send(text: "Pick me")
        let id = try #require(result.id)
        let list = session.conversations()
        #expect(list.selectedId() == nil)
        list.select(id: id)
        #expect(list.selectedId() == id)
        list.select(id: nil)
        #expect(list.selectedId() == nil)
    }
}
