import ArutBindings
import Testing

@MainActor
struct SessionTests {
    @Test
    func sendUpdatesObservedTranscriptAndConversationList() async throws {
        let session = createProductSession(pendingScopeId: "swift-test")
        let initialized = try await session.initialize()
        #expect(initialized)
        let chat = session.chat()
        let composer = chat.composer()
        let list = session.conversations()
        let transcript = ObservableState(read: { chat.messagesAfter(afterId: 0) }, subscribe: { callback in
            let subscription = chat.chatChanges(callback: callback)
            return { subscription.cancel() }
        })
        defer { transcript.stopObserving() }

        _ = try await composer.replace(text: "Hello from Swift")
        #expect(composer.state().text == "Hello from Swift")
        let result = try await chat.send(text: composer.state().text)
        #expect(result.error == nil)
        // Observation deliberately coalesces onto the main actor. Wait for the
        // published value with a deadline so a broken subscription fails.
        let clock = ContinuousClock()
        let deadline = clock.now.advanced(by: .seconds(5))
        while transcript.state.count < 2, clock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(transcript.state.count == 2)
        #expect(transcript.state.first?.text == "Hello from Swift")
        #expect(list.state().count == 1)
        #expect(composer.state().text == "")
        let id = try #require(result.id)
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

}
