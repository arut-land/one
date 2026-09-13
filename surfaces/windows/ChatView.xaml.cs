using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using System;
using System.Collections.ObjectModel;
using System.Threading;
using System.Threading.Tasks;

namespace Arut.Surface.Windows;

public sealed partial class ChatView : UserControl
{
    private readonly ProductSessionHandle session;
    private readonly ConversationsHandle list;
    private readonly ObservableCollection<ChatMessage> messages = new();
    private ChatHandle chat;
    private ComposerHandle composer;
    private IDisposable? chatSubscription;
    private IDisposable? composerSubscription;
    private IDisposable? listSubscription;
    private CancellationTokenSource? following;
    private int generation;

    public ChatView(ProductSessionHandle session)
    {
        this.session = session;
        list = session.Conversations();
        chat = session.Chat();
        composer = chat.Composer();
        InitializeComponent();
        NewConversationButton.Content = L10n.ActionNewConversation();
        SendButton.Content = L10n.ActionSend();
        Composer.PlaceholderText = L10n.ComposerPlaceholder();
        Transcript.ItemsSource = messages;
        Loaded += (_, _) => Observe();
        Unloaded += (_, _) => StopObserving();
    }

    private void Bind(ChatHandle next)
    {
        StopObserving();
        composer.Dispose(); chat.Dispose();
        chat = next; composer = chat.Composer();
        messages.Clear();
        if (IsLoaded) Observe();
    }

    private void Observe()
    {
        var observedGeneration = generation;
        void Enqueue(Action refresh) => DispatcherQueue.TryEnqueue(() =>
        {
            if (observedGeneration == generation) refresh();
        });
        chatSubscription = chat.ChatChanges(_ => Enqueue(() =>
        {
            foreach (var message in chat.MessagesAfter(messages.Count == 0 ? 0UL : messages[messages.Count - 1].Id)) messages.Add(message);
            UpdateError();
        }));
        composerSubscription = composer.ComposerChanges(_ => Enqueue(() =>
        {
            if (Composer.Text != composer.State().Text) Composer.Text = composer.State().Text;
            UpdateError();
        }));
        listSubscription = list.ListChanges(_ => Enqueue(() => History.ItemsSource = list.State()));
        following = new CancellationTokenSource();
        _ = Follow(composer, following.Token);
    }

    private void StopObserving()
    {
        generation++;
        chatSubscription?.Dispose(); composerSubscription?.Dispose(); listSubscription?.Dispose();
        following?.Cancel(); following?.Dispose(); following = null;
    }

    private static async Task Follow(ComposerHandle current, CancellationToken cancellation)
    {
        try
        {
            await current.Initialize(cancellation);
            cancellation.ThrowIfCancellationRequested();
            await current.Follow(cancellation);
        }
        catch (OperationCanceledException) when (cancellation.IsCancellationRequested) { }
    }

    private void UpdateError()
    {
        var message = chat.State().Error is { } chatError
            ? Strings.Describe(chatError)
            : composer.State().Error is { } composerError
                ? Strings.Describe(composerError)
                : null;
        ErrorText.Text = message ?? string.Empty;
        ErrorText.Visibility = message is null ? Visibility.Collapsed : Visibility.Visible;
    }
    private async void Send(object sender, RoutedEventArgs args) => await chat.Send(composer.State().Text);
    private async void ReplaceDraft(object sender, TextChangedEventArgs args) { if (Composer.Text != composer.State().Text) await composer.Replace(Composer.Text); }
    private void NewChat(object sender, RoutedEventArgs args) => Bind(session.NewChat());
    private void SelectChat(object sender, SelectionChangedEventArgs args) { if (History.SelectedItem is ChatSummary summary && summary.Id != chat.State().Id && session.SelectChat(summary.Id) is ChatHandle next) Bind(next); }
}
