#if os(macOS)
import SwiftUI

private struct SessionStateKey: FocusedValueKey {
    typealias Value = SessionState
}

extension FocusedValues {
    var conversations: SessionState? {
        get { self[SessionStateKey.self] }
        set { self[SessionStateKey.self] = newValue }
    }
}

public struct ConversationCommands: Commands {
    @FocusedValue(\.conversations) private var state

    public init() {}

    public var body: some Commands {
        CommandGroup(replacing: .newItem) {
            Button(L10n.actionNewConversation()) { state?.newConversation() }
                .keyboardShortcut("n", modifiers: .command)
                .disabled(state == nil)
        }
        CommandGroup(after: .textEditing) {
            Button(L10n.actionSearchConversations()) { state?.focusSearch() }
                .keyboardShortcut("f", modifiers: .command)
                .disabled(state == nil)
            Button(L10n.actionFocusComposer()) { state?.focusComposer() }
                .keyboardShortcut("l", modifiers: .command)
                .disabled(state == nil)
        }
        CommandMenu(L10n.labelConversations()) {
            Button(L10n.actionNextConversation()) { state?.move(1) }
                .keyboardShortcut(.tab, modifiers: .control)
                .disabled(state == nil)
            Button(L10n.actionPreviousConversation()) { state?.move(-1) }
                .keyboardShortcut(.tab, modifiers: [.control, .shift])
                .disabled(state == nil)
        }
        SidebarCommands()
    }
}
#endif
