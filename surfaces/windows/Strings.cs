namespace Arut.Surface.Windows;

/// <summary>
/// The static labels XAML cannot reach through `x:Uid`: WinUI applies every
/// `<uid>.<property>` entry the generated `.resw` defines, which would replace
/// an icon button's content or prefill a search box, so those bind here
/// instead. `x:Uid` carries the labels whose element has no such property.
/// </summary>
internal static class Labels
{
    public static string AppName => L10n.Get(L10n.AppName);
    public static string Search => L10n.Get(L10n.ConversationSearchPlaceholder);
    public static string Conversations => L10n.Get(L10n.LabelConversations);
    public static string NewConversation => L10n.Get(L10n.ActionNewConversation);
    public static string NewConversationShortcut =>
        L10n.Get(L10n.ActionNewConversationShortcut, "Ctrl+N");
    public static string EmptyTitle => L10n.Get(L10n.ChatEmptyTitle);
    public static string EmptyHint => L10n.Get(L10n.ChatEmptyHint);
    public static string Composer => L10n.Get(L10n.ComposerPlaceholder);
    public static string ComposerHint => L10n.Get(L10n.ComposerHintMultiline);
    public static string Send => L10n.Get(L10n.ActionSend);
    public static string Latest => L10n.Get(L10n.ActionScrollToLatest);
    public static string Copy => L10n.Get(L10n.ActionCopyMessage);
    public static string Unread => L10n.Get(L10n.LabelUnreadMessages);
    public static string RenameConversation => L10n.Get(L10n.ActionRenameConversation);
    public static string DeleteConversation => L10n.Get(L10n.ActionDeleteConversation);
    public static string RenameConversationTitle => L10n.Get(L10n.ConversationRenameTitle);
    public static string DeleteConversationTitle => L10n.Get(L10n.ConversationDeleteTitle);
    public static string DeleteConversationMessage => L10n.Get(L10n.ConversationDeleteMessage);
    public static string Cancel => L10n.Get(L10n.ActionCancel);
    public static string Save => L10n.Get(L10n.ActionSave);
}
