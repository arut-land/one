import SwiftUI

#if os(macOS)
/// What the Help menu opens: the keyboard shortcuts this app defines, in one
/// place, so `Help` is a real menu rather than an empty one (macOS rule 1.1).
public struct ShortcutsView: View {
    /// The window `ConversationCommands` opens.
    public static let windowId = "arut-shortcuts"
    /// Its title, resolved here because `L10n` is this module's.
    public static var windowTitle: String { L10n.labelKeyboardShortcuts() }

    public init() {}

    public var body: some View {
        Form {
            row(L10n.actionNewConversation(), "⌘N")
            row(L10n.actionSearchConversations(), "⌘F")
            row(L10n.actionFocusComposer(), "⌘L")
            row(L10n.actionNextConversation(), "⌃⇥")
            row(L10n.actionPreviousConversation(), "⌃⇧⇥")
            row(L10n.actionSendMessage(), "⌘↩")
        }
        .formStyle(.grouped)
        .frame(minWidth: 380, minHeight: 300)
    }

    private func row(_ label: String, _ shortcut: String) -> some View {
        LabeledContent(label) {
            Text(shortcut).monospaced().foregroundStyle(.secondary)
        }
    }
}
#endif
