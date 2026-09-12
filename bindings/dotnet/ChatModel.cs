using System;
using System.ComponentModel;
using System.Threading;
using System.Threading.Tasks;
using Arut.Ffi;

namespace Arut.Bindings;

public sealed class ChatModel : INotifyPropertyChanged, IDisposable
{
    private readonly ProductSessionHandle session;
    private readonly CancellationTokenSource lifetime = new();
    private ChatHandle core;
    private ComposerHandle composer;
    private ObservableState<ChatState> chatState;
    private ObservableState<ComposerState> composerState;
    private Task draftWrites = Task.CompletedTask;
    private string draft = "";
    private long epoch;

    public ChatModel()
    {
        session = Arut_ffi.CreateProductSession("", "local-demo");
        core = session.Chat();
        composer = core.Composer();
        chatState = new(core.State(), core.State, core.ChatChanges);
        composerState = new(composer.State(), composer.State, composer.ComposerChanges);
        chatState.Changed += ChatChanged;
        composerState.Changed += ComposerChanged;
        _ = InitializeAndPoll(Volatile.Read(ref epoch), lifetime.Token);
    }

    public ChatState State => chatState.Value;
    public ComposerState Composer => composerState.Value;
    public ChatSummary[] History => session.ChatSummaries();
    public string SelectedChatId => core.Id();

    public string Draft
    {
        get => draft;
        set
        {
            if (draft == value) return;
            draft = value;
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(Draft)));
            var expectedEpoch = Volatile.Read(ref epoch);
            draftWrites = WriteDraftAfter(draftWrites, composer, value, expectedEpoch);
        }
    }

    public async Task Send()
    {
        var expectedEpoch = Volatile.Read(ref epoch);
        var targetComposer = composer;
        var targetChat = core;
        await draftWrites;
        var message = draft.Trim();
        if (message.Length == 0 || State.Status == ChatStatus.Sending || expectedEpoch != Volatile.Read(ref epoch)) return;
        await targetChat.Send(message);
        if (expectedEpoch == Volatile.Read(ref epoch))
        {
            ApplyComposer(targetComposer.State());
            PublishChatProperties();
        }
    }

    public void NewChat() => Bind(session.NewChat());

    public void SelectChat(string chatId) => Bind(session.SelectChat(chatId));

    public string[] ChatIds() => session.ChatIds();

    public void Dispose()
    {
        lifetime.Cancel();
        Interlocked.Increment(ref epoch);
        chatState.Dispose();
        composerState.Dispose();
        composer.Dispose();
        core.Dispose();
        session.Dispose();
        lifetime.Dispose();
    }

    public event PropertyChangedEventHandler? PropertyChanged;

    private void Bind(ChatHandle next)
    {
        Interlocked.Increment(ref epoch);
        chatState.Changed -= ChatChanged;
        composerState.Changed -= ComposerChanged;
        chatState.Dispose();
        composerState.Dispose();
        composer.Dispose();
        core.Dispose();
        core = next;
        composer = core.Composer();
        chatState = new(core.State(), core.State, core.ChatChanges);
        composerState = new(composer.State(), composer.State, composer.ComposerChanges);
        chatState.Changed += ChatChanged;
        composerState.Changed += ComposerChanged;
        ApplyComposer(composer.State());
        ChatChanged();
        _ = InitializeAndPoll(Volatile.Read(ref epoch), lifetime.Token);
    }

    private async Task WriteDraftAfter(Task previous, ComposerHandle target, string text, long expectedEpoch)
    {
        await previous;
        if (expectedEpoch != Volatile.Read(ref epoch)) return;
        await target.Replace(text);
    }

    private async Task InitializeAndPoll(long expectedEpoch, CancellationToken cancellationToken)
    {
        var target = composer;
        try
        {
            await target.Initialize(cancellationToken);
            while (!cancellationToken.IsCancellationRequested && expectedEpoch == Volatile.Read(ref epoch))
            {
                await Task.Delay(500, cancellationToken);
                if (expectedEpoch != Volatile.Read(ref epoch)) return;
                await draftWrites;
                var state = await target.SyncOnce(cancellationToken);
                if (expectedEpoch == Volatile.Read(ref epoch)) ApplyComposer(state);
            }
        }
        catch (OperationCanceledException) { }
    }

    private void ChatChanged() =>
        PublishChatProperties();

    private void PublishChatProperties()
    {
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(State)));
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(History)));
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(SelectedChatId)));
    }

    private void ComposerChanged() => ApplyComposer(composerState.Value);

    private void ApplyComposer(ComposerState state)
    {
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(Composer)));
        if (!draftWrites.IsCompleted) return;
        if (draft == state.Text) return;
        draft = state.Text;
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(Draft)));
    }
}
