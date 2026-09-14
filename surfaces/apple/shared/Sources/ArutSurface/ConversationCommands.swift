#if os(macOS)
import SwiftUI

extension FocusedValues {
    // `@Entry` writes the key type and both accessors. It expands at compile
    // time, so it costs nothing at this deployment floor.
    @Entry var conversations: SessionState?
}

public struct ConversationCommands: Commands {
    @FocusedValue(\.conversations) private var state
    @Environment(\.openWindow) private var openWindow

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
        // Every Mac app has a Help menu; the default one points at a help book
        // this app does not ship, so it opens the shortcut reference instead.
        CommandGroup(replacing: .help) {
            Button(L10n.actionAppHelp()) { openWindow(id: ShortcutsView.windowId) }
                .keyboardShortcut("/", modifiers: [.command, .shift])
        }
        SidebarCommands()
    }
}
#endif
