using System.Collections.ObjectModel;
using System.ComponentModel;
using Arut.Bindings;
using Microsoft.UI.Dispatching;

namespace Arut.Surface.Windows;

// Owns one conversation's native handles. Commands drain before those handles close.
public sealed class ConversationModel : INotifyPropertyChanged, IAsyncDisposable
{
    private readonly ChatHandle chat;
    private readonly ComposerHandle composer;
    private readonly CancellationTokenSource lifetime = new();
    private readonly ObservableState<ChatState> state;
    private readonly ObservableState<ComposerState> draftState;
    private readonly Task following;
    private readonly ConversationCommands commands;
    private long draftVersion;
    private long appliedDraftVersion;
    private bool sending;
    private bool disposed;
    private bool active = true;
    private string draft = "";
    private string? failure;
    private bool holdReplies;
    private bool hasDeferredReplies;
    private bool revealReplies;

    public ConversationModel(ChatHandle chat, DispatcherQueue dispatcher)
    {
        this.chat = chat;
        composer = chat.Composer();
        bool Dispatch(Action action) => dispatcher.TryEnqueue(() => action());
        state = new(chat.State, chat.ChatChanges, Dispatch);
        draftState = new(composer.State, composer.ComposerChanges, Dispatch);
        state.Changed += RefreshTranscript;
        draftState.Changed += RefreshDraft;
        Refresh();
        var initialized = Initialize();
        commands = new(initialized);
        following = Follow(initialized);
    }

    public ObservableCollection<MessageRow> Messages { get; } = new();
    public string? Id => state.Value.Id;
    public bool CanSend => !disposed && !sending && !string.IsNullOrWhiteSpace(draft);
    public bool IsSending => sending || state.Value.Status == ChatStatus.Sending;
    public bool IsEmpty => Messages.Count == 0;
    public MessageRow? ReadingAnchor { get; set; }
    public bool ReadingAtTop { get; set; } = true;
    public double ReadingAnchorOffset { get; set; }
    public bool FollowLatest { get; set; } = true;
    public string Error =>
        failure
        ?? (
            state.Value.Error is { } error ? Strings.Describe(error)
            : draftState.Value.Error is { } draftError ? Strings.Describe(draftError)
            : ""
        );
    public bool HasError => Error.Length > 0;
    public string Draft
    {
        get => draft;
        set
        {
            if (disposed || draft == value)
                return;
            draft = value;
            var version = ++draftVersion;
            _ = Enqueue(
                async () =>
                {
                    try
                    {
                        await composer.Replace(value, lifetime.Token);
                    }
                    finally
                    {
                        appliedDraftVersion = version;
                    }
                },
                edit: true
            );
            Notify(nameof(CanSend));
        }
    }

    public event PropertyChangedEventHandler? PropertyChanged;

    private void Notify(params string[] properties)
    {
        foreach (var property in properties)
            PropertyChanged?.Invoke(this, new(property));
    }

    private void NotifyStatus() =>
        Notify(nameof(Id), nameof(IsSending), nameof(CanSend), nameof(Error), nameof(HasError));

    public void SetActive(bool value)
    {
        if (disposed || active == value)
            return;
        active = value;
        if (active)
        {
            state.Observe(chat.State, chat.ChatChanges);
            draftState.Observe(composer.State, composer.ComposerChanges);
            Refresh();
        }
        else
        {
            StopReplyMotion();
            state.StopObserving();
            draftState.StopObserving();
        }
    }

    private void Refresh()
    {
        RefreshTranscript();
        RefreshDraft();
    }

    internal void HoldReplies()
    {
        holdReplies = true;
        revealReplies = false;
    }

    internal void StopReplyMotion()
    {
        revealReplies = false;
        foreach (var message in Messages)
            message.RevealOnLoad = false;
    }

    internal void ReleaseReplies(bool animate)
    {
        holdReplies = hasDeferredReplies = false;
        revealReplies = animate;
        RefreshTranscript();
    }

    private void RefreshTranscript()
    {
        if (disposed)
            return;
        ulong cursor = Messages.Count == 0 ? 0 : Messages[^1].Id;
        if (!hasDeferredReplies && state.Value.LastMessageId > cursor)
        {
            foreach (var message in chat.MessagesAfter(cursor))
            {
                // Keep accepted replies in Rust until the outgoing transition ends.
                // Not inserting them also avoids reserving blank transcript space.
                if (holdReplies && message.Role != ChatRole.User)
                {
                    hasDeferredReplies = true;
                    break;
                }
                Messages.Add(
                    new(message) { RevealOnLoad = revealReplies && message.Role != ChatRole.User }
                );
            }
            Notify(nameof(IsEmpty));
        }
        NotifyStatus();
    }

    private void RefreshDraft()
    {
        if (disposed)
            return;
        // Only a fresh composer snapshot may acknowledge an edit. Chat revisions
        // can arrive first and still have an older cached composer snapshot.
        if (appliedDraftVersion == draftVersion && draft != draftState.Value.Text)
        {
            draft = draftState.Value.Text;
            Notify(nameof(Draft));
        }
        NotifyStatus();
    }

    public Task SendAsync()
    {
        if (!CanSend)
            return Task.CompletedTask;
        var text = draft;
        sending = true;
        NotifyStatus();
        return Enqueue(async () =>
        {
            try
            {
                await chat.Send(text, lifetime.Token);
            }
            finally
            {
                sending = false;
            }
        });
    }

    private Task Enqueue(Func<Task> action, bool edit = false)
    {
        return commands.Enqueue(Run, edit);
        async Task Run()
        {
            try
            {
                lifetime.Token.ThrowIfCancellationRequested();
                failure = null;
                await action();
            }
            catch (OperationCanceledException) when (lifetime.IsCancellationRequested) { }
            catch (Exception exception)
            {
                System.Diagnostics.Trace.TraceError(exception.ToString());
                failure = L10n.NodeFailureInternal();
            }
            finally
            {
                if (!disposed)
                {
                    // Read after completion, independently of callback delivery timing.
                    if (!edit)
                        state.RefreshNow();
                    // Reconcile unchanged snapshots too, but never a rejected read's cache.
                    if (draftState.RefreshNow())
                        RefreshDraft();
                }
            }
        }
    }

    private async Task Initialize()
    {
        try
        {
            await composer.Initialize(lifetime.Token);
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested) { }
        catch (Exception exception)
        {
            System.Diagnostics.Trace.TraceError(exception.ToString());
            if (!disposed)
            {
                failure = L10n.NodeFailureInternal();
                NotifyStatus();
            }
        }
    }

    private async Task Follow(Task initialized)
    {
        await initialized;
        try
        {
            await composer.Follow(lifetime.Token);
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested) { }
        catch (Exception exception)
        {
            System.Diagnostics.Trace.TraceError(exception.ToString());
            if (!disposed)
            {
                failure = L10n.NodeFailureInternal();
                NotifyStatus();
            }
        }
    }

    public async ValueTask DisposeAsync()
    {
        if (disposed)
            return;
        disposed = true;
        state.Dispose();
        draftState.Dispose();
        lifetime.Cancel();
        await commands.Completion;
        await following;
        composer.Dispose();
        chat.Dispose();
        lifetime.Dispose();
    }
}
