using CommunityToolkit.Mvvm.ComponentModel;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Arut.Surface.Windows;

/// <summary>A stable native row whose displayed fields update in place.</summary>
public sealed partial class ConversationRow : ObservableObject
{
    public ConversationRow(ChatSummary summary)
    {
        Id = summary.Id;
        Title = summary.Title;
        Preview = summary.Preview;
        Unread = summary.Unread;
    }

    public string Id { get; }

    [ObservableProperty]
    public partial string Title { get; set; }

    [ObservableProperty]
    public partial string Preview { get; set; }

    [ObservableProperty]
    public partial bool Unread { get; set; }

    public void Update(ChatSummary summary)
    {
        Title = summary.Title;
        Preview = summary.Preview;
        Unread = summary.Unread;
    }

    public override string ToString() => Title;
}

/// <summary>
/// One transcript row. Rust decides the grouping (<c>StartsTimeGroup</c>,
/// <c>StartsSpeakerGroup</c>, <c>EndsSpeakerGroup</c>) and what instant the row
/// carries; Windows decides how a date reads here.
/// </summary>
public sealed record MessageRow(
    ulong Id,
    string Text,
    bool IsOutgoing,
    bool StartsGroup,
    bool EndsGroup,
    string Time,
    string FullTime,
    string TimeGroup
)
{
    public static MessageRow From(ChatMessage message)
    {
        var timestamp = Arut.Bindings.Time.AcceptedAt(message.AcceptedAtMs);
        var previousTimeGroup = message.PreviousTimeGroupAtMs is { } previousAt
            ? Arut.Bindings.Time.AcceptedAt(previousAt)
            : null;
        var timeGroup = message.StartsTimeGroup && timestamp is { } current
            ? previousTimeGroup is { } previous && previous.Date == current.Date
                ? current.ToString("t")
                : current.ToString("f")
            : "";
        return new MessageRow(
            message.Id,
            message.Text,
            message.Role == ChatRole.User,
            message.StartsSpeakerGroup,
            message.EndsSpeakerGroup,
            timestamp?.ToString("t") ?? "",
            timestamp?.ToString("f") ?? "",
            timeGroup
        );
    }

    public string Author =>
        IsOutgoing ? L10n.Get(L10n.ChatRoleYou) : L10n.Get(L10n.ChatRoleAssistant);
    // x:Bind converts bool to Visibility itself, so no converter is needed.
    public bool HasTime => Time.Length > 0;
    public bool HasTimeGroup => TimeGroup.Length > 0;
    public Thickness Spacing => new(0, StartsGroup ? 10 : 2, 0, EndsGroup ? 8 : 0);

    public override string ToString() => $"{Author}: {Text}. {FullTime}";
}

/// <summary>Outgoing and incoming bubbles are two templates, not one template
/// plus visual states, so each keeps its own theme brushes in XAML.</summary>
public sealed class MessageTemplateSelector : DataTemplateSelector
{
    public DataTemplate? Incoming { get; set; }
    public DataTemplate? Outgoing { get; set; }

    protected override DataTemplate? SelectTemplateCore(object item) =>
        item is MessageRow { IsOutgoing: true } ? Outgoing : Incoming;

    protected override DataTemplate? SelectTemplateCore(object item, DependencyObject container) =>
        SelectTemplateCore(item);
}
