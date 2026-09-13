#if os(macOS)
import SwiftUI

private struct NewConversationKey: FocusedValueKey {
    typealias Value = () -> Void
}

private struct FocusComposerKey: FocusedValueKey {
    typealias Value = () -> Void
}

private struct SearchConversationsKey: FocusedValueKey {
    typealias Value = () -> Void
}

private struct MoveConversationKey: FocusedValueKey {
    typealias Value = (Int) -> Void
}

extension FocusedValues {
    var newConversation: (() -> Void)? {
        get { self[NewConversationKey.self] }
        set { self[NewConversationKey.self] = newValue }
    }
    var focusComposer: (() -> Void)? {
        get { self[FocusComposerKey.self] }
        set { self[FocusComposerKey.self] = newValue }
    }
    var searchConversations: (() -> Void)? {
        get { self[SearchConversationsKey.self] }
        set { self[SearchConversationsKey.self] = newValue }
    }
    var moveConversation: ((Int) -> Void)? {
        get { self[MoveConversationKey.self] }
        set { self[MoveConversationKey.self] = newValue }
    }
}

public struct ConversationCommands: Commands {
    @FocusedValue(\.newConversation) private var newConversation
    @FocusedValue(\.focusComposer) private var focusComposer
    @FocusedValue(\.searchConversations) private var searchConversations
    @FocusedValue(\.moveConversation) private var moveConversation

    public init() {}

    public var body: some Commands {
        CommandGroup(replacing: .newItem) {
            Button(L10n.actionNewConversation()) { newConversation?() }
                .keyboardShortcut("n", modifiers: .command)
                .disabled(newConversation == nil)
        }
        CommandGroup(after: .textEditing) {
            if #available(macOS 15.0, *) {
                Button(L10n.actionSearchConversations()) { searchConversations?() }
                    .keyboardShortcut("f", modifiers: .command)
                    .disabled(searchConversations == nil)
            }
            Button(L10n.actionFocusComposer()) { focusComposer?() }
                .keyboardShortcut("l", modifiers: .command)
                .disabled(focusComposer == nil)
        }
        CommandMenu(L10n.labelConversations()) {
            Button(L10n.actionNextConversation()) { moveConversation?(1) }
                .keyboardShortcut(.tab, modifiers: .control)
                .disabled(moveConversation == nil)
            Button(L10n.actionPreviousConversation()) { moveConversation?(-1) }
                .keyboardShortcut(.tab, modifiers: [.control, .shift])
                .disabled(moveConversation == nil)
        }
        SidebarCommands()
    }
}
#endif
