using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;

namespace Arut.Surface.Windows;

/// <summary>
/// The conversation list and which conversation this session shows. Rust owns
/// the selection, narrows the list to the query and names the selected
/// conversation, so what is left here is which view model is alive and how many
/// of them stay that way.
/// </summary>
public sealed partial class ChatViewModel : ObservableObject, IAsyncDisposable
{
    /// <summary>How many visited conversations keep their handles and pumps.
    /// Each costs two FFI handles and three pumps; Rust keeps every draft, so an
    /// evicted conversation loses nothing but its warm start.</summary>
    private const int Resident = 8;

    private readonly ProductSessionHandle session;
    private readonly ConversationsHandle list;
    private readonly Projection<ChatSummary[]> summaries;
    private readonly EchoGuard echo = new();
    /// <summary>Most recently shown last: the eviction order.</summary>
    private readonly List<ConversationViewModel> resident = [];
    private ConversationViewModel pending;
    private bool disposed;

    public ChatViewModel(ProductSessionHandle session)
    {
        this.session = session;
        list = session.Conversations();
        summaries = new Projection<ChatSummary[]>(list.State, list.ListChanges);
        pending = Track(new ConversationViewModel(session.Chat()));
        Conversation = pending;
        Search = list.Query();
        Title = L10n.Get(L10n.ActionNewConversation);
        summaries.PropertyChanged += (_, _) => Refresh();
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

    public ObservableCollection<ConversationRow> Conversations { get; } = [];
    public bool IsHistoryEmpty => Conversations.Count == 0;
    public string HistoryEmptyLabel =>
        Search.Length == 0 ? L10n.Get(L10n.ChatHistoryEmpty) : L10n.Get(L10n.ConversationSearchEmpty);

    public void Start() => summaries.Start();

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
        // One view model per resident conversation, so a pending draft command
        // can never be redirected to another conversation mid-flight.
        var model = resident.Find(candidate => candidate.Id == id);
        if (model is null)
        {
            if (session.SelectChat(id) is not { } handle)
                return;
            model = Track(new ConversationViewModel(handle));
        }
        list.Select(id);
        Show(model);
    }

    // Rust narrows the list; writing the query is the whole search here.
    partial void OnSearchChanged(string value)
    {
        if (echo.IsApplying)
            return;
        list.SetQuery(value);
        summaries.Refresh();
        OnPropertyChanged(nameof(HistoryEmptyLabel));
    }

    partial void OnSelectedIndexChanged(int value)
    {
        if (echo.IsApplying || value < 0 || value >= Conversations.Count)
            return;
        Open(Conversations[value].Id);
    }

    private void Refresh()
    {
        if (disposed)
            return;
        var rows = summaries.Value;
        var selected = list.SelectedId();
        using (echo.Applying())
        {
            // The rows that stayed keep their containers, so the pane keeps its
            // scroll offset and the selection never flickers through -1.
            // FFI summaries contain arrays with reference equality. Bind stable
            // observable rows, so a new projection does not replace every item.
            var existing = Conversations.ToDictionary(row => row.Id);
            var next = rows.Select(summary =>
            {
                if (!existing.TryGetValue(summary.Id, out var row))
                    return new ConversationRow(summary);
                row.Update(summary);
                return row;
            }).ToArray();
            Reconcile.Apply(Conversations, next, row => row.Id);
            Search = list.Query();
            SelectedIndex =
                selected is null ? -1 : Array.FindIndex(rows, row => row.Id == selected);
        }
        // `null` means no conversation is named yet, so the surface shows its
        // own label -- the only part of the title that is localized.
        Title = list.Title() ?? L10n.Get(L10n.ActionNewConversation);
        OnPropertyChanged(nameof(IsHistoryEmpty));
        OnPropertyChanged(nameof(HistoryEmptyLabel));
    }

    private ConversationViewModel Track(ConversationViewModel model)
    {
        resident.Add(model);
        model.Start();
        return model;
    }

    private void Show(ConversationViewModel model)
    {
        Conversation = model;
        resident.Remove(model);
        resident.Add(model);
        Evict();
        Refresh();
    }

    /// <summary>Drops the least recently shown conversations past the cap; the
    /// pending one and the one on screen always stay.</summary>
    private void Evict()
    {
        while (resident.Count > Resident)
        {
            var stale = resident.Find(model => model != Conversation && model != pending);
            if (stale is null)
                return;
            resident.Remove(stale);
            _ = stale.DisposeAsync();
        }
    }

    public async ValueTask DisposeAsync()
    {
        if (disposed)
            return;
        disposed = true;
        await summaries.DisposeAsync();
        foreach (var model in resident)
            await model.DisposeAsync();
        resident.Clear();
        list.Dispose();
    }
}
