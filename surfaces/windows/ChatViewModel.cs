using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;

namespace Arut.Surface.Windows;

/// <summary>
/// The conversation list and which conversation this session shows. Rust owns
/// the selection, so every surface on one session follows the same one; the
/// only thing kept here is the visible filter over the list it publishes.
/// </summary>
public sealed partial class ChatViewModel : ObservableObject, IAsyncDisposable
{
    private readonly ProductSessionHandle session;
    private readonly ConversationsHandle list;
    private readonly CancellationTokenSource lifetime = new();
    private readonly List<ConversationViewModel> all = [];
    private ConversationViewModel pending;
    private ChatSummary[] summaries = [];
    private Task running = Task.CompletedTask;
    private bool applying;
    private bool disposed;

    public ChatViewModel(ProductSessionHandle session)
    {
        this.session = session;
        list = session.Conversations();
        pending = Track(new ConversationViewModel(session.Chat()));
        Conversation = pending;
        Search = "";
        Title = L10n.Get(L10n.ActionNewConversation);
        Refresh();
    }

    [ObservableProperty]
    public partial ConversationViewModel Conversation { get; set; }

    [ObservableProperty]
    public partial string Search { get; set; }

    [ObservableProperty]
    public partial int SelectedIndex { get; set; }

    [ObservableProperty]
    public partial string Title { get; set; }

    public ObservableCollection<ChatSummary> Conversations { get; } = [];
    public bool IsHistoryEmpty => Conversations.Count == 0;
    public string HistoryEmptyLabel =>
        Search.Length == 0 ? L10n.Get(L10n.ChatHistoryEmpty) : L10n.Get(L10n.ConversationSearchEmpty);

    public void Start() => running = PumpAsync();

    public void NewConversation()
    {
        if (pending.Id is not null)
            pending = Track(new ConversationViewModel(session.NewChat()));
        list.Select(null);
        Show(pending);
    }

    public void Open(string id)
    {
        if (id == Conversation.Id)
            return;
        // One view model per visited conversation, so a pending draft command
        // can never be redirected to another conversation mid-flight.
        var model = all.Find(candidate => candidate.Id == id);
        if (model is null)
        {
            if (session.SelectChat(id) is not { } handle)
                return;
            model = Track(new ConversationViewModel(handle));
        }
        list.Select(id);
        Show(model);
    }

    partial void OnSearchChanged(string value)
    {
        Refresh();
        OnPropertyChanged(nameof(HistoryEmptyLabel));
    }

    partial void OnSelectedIndexChanged(int value)
    {
        if (applying || value < 0 || value >= Conversations.Count)
            return;
        Open(Conversations[value].Id);
    }

    private async Task PumpAsync()
    {
        try
        {
            await foreach (var _ in list.ListChanges(lifetime.Token))
                Refresh();
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested) { }
    }

    // The whole filter. Rust publishes the ordered list; this chooses which of
    // its rows are visible and where the selection sits among them.
    private void Refresh()
    {
        if (disposed)
            return;
        summaries = list.State();
        var rows = summaries
            .Where(row => row.Title.Contains(Search, StringComparison.CurrentCultureIgnoreCase))
            .ToArray();
        applying = true;
        Conversations.Clear();
        foreach (var row in rows)
            Conversations.Add(row);
        var selected = list.SelectedId();
        SelectedIndex = selected is null ? -1 : Array.FindIndex(rows, row => row.Id == selected);
        applying = false;
        var current = Array.Find(summaries, row => row.Id == selected);
        Title = string.IsNullOrEmpty(current.Title)
            ? L10n.Get(L10n.ActionNewConversation)
            : current.Title;
        OnPropertyChanged(nameof(IsHistoryEmpty));
    }

    private ConversationViewModel Track(ConversationViewModel model)
    {
        all.Add(model);
        model.Start();
        return model;
    }

    private void Show(ConversationViewModel model)
    {
        Conversation = model;
        Refresh();
    }

    public async ValueTask DisposeAsync()
    {
        if (disposed)
            return;
        disposed = true;
        lifetime.Cancel();
        try
        {
            await running;
        }
        catch (OperationCanceledException) { }
        foreach (var model in all)
            await model.DisposeAsync();
        list.Dispose();
        lifetime.Dispose();
    }
}
