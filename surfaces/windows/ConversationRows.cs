using System.ComponentModel;
using Arut.Bindings;

namespace Arut.Surface.Windows;

public sealed class MessageRow
{
    internal bool RevealOnLoad { get; set; }

    public MessageRow(ChatMessage message)
    {
        Id = message.Id;
        Text = message.Text;
        IsOutgoing = message.Role == ChatRole.User;
        var acceptedAtMs = message.AcceptedAtMs;
        var timestamp =
            acceptedAtMs > 0 && acceptedAtMs <= 253402300799999UL
                ? DateTimeOffset.FromUnixTimeMilliseconds((long)acceptedAtMs).ToLocalTime()
                : (DateTimeOffset?)null;
        StartsGroup = message.StartsSpeakerGroup;
        Time = timestamp?.ToString("t") ?? "";
        FullTime = timestamp?.ToString("f") ?? "";
        TimeGroup = message.StartsTimeGroup ? FullTime : "";
    }

    public ulong Id { get; }
    public string Text { get; }
    public bool IsOutgoing { get; }
    public bool StartsGroup { get; }
    public string Time { get; }
    public string FullTime { get; }
    public string TimeGroup { get; }
    public string Author => IsOutgoing ? L10n.ChatRoleYou() : L10n.ChatRoleAssistant();

    public override string ToString() => $"{Author}: {Text}. {FullTime}";
}

// WinUI's XAML compiler generates setters for record structs. Expose read-only
// presentation properties without changing the generated Rust value types.
public sealed class ConversationRow(ChatSummary summary) : INotifyPropertyChanged
{
    public string Id => summary.Id;
    public string Title => summary.Title;
    public string Preview => summary.Preview;
    public event PropertyChangedEventHandler? PropertyChanged;

    // Keep the native item container and its focus when a title or preview changes.
    internal void Update(ChatSummary next)
    {
        var titleChanged = summary.Title != next.Title;
        var previewChanged = summary.Preview != next.Preview;
        summary = next;
        if (titleChanged)
            PropertyChanged?.Invoke(this, new(nameof(Title)));
        if (previewChanged)
            PropertyChanged?.Invoke(this, new(nameof(Preview)));
    }

    public override string ToString() => Title;
}
