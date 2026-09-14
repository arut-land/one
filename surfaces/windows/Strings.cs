namespace Arut.Surface.Windows;

/// <summary>
/// The core names an error by its Fluent message id and hands over the
/// arguments that id takes; no sentence crosses the boundary and the core still
/// learns no locale (ADR 0016, ADR 0022). The lookup and the language
/// resolution belong to <see cref="L10n"/> and the app's PRI resources.
/// Fluent ids are hyphenated; resw names are the same ids with underscores.
/// </summary>
internal static class Strings
{
    public static string Describe(string? key, string[] arguments) =>
        key is null ? "" : L10n.Get(key.Replace('-', '_'), arguments);
}

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
}
