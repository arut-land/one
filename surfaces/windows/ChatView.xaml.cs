using System.Collections.ObjectModel;
using Arut.Bindings;
using CommunityToolkit.WinUI;
using global::Windows.ApplicationModel.DataTransfer;
using global::Windows.System;
using global::Windows.UI.Core;
using global::Windows.UI.ViewManagement;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;

namespace Arut.Surface.Windows;

public sealed partial class ChatView : UserControl, IAsyncDisposable
{
    private readonly ProductSessionHandle session;
    private readonly ConversationsHandle list;
    private readonly ObservableState<ChatSummary[]> history;
    private readonly List<ConversationModel> conversations = new();
    private ConversationModel current;
    private bool disposed;
    private bool composing;
    private readonly UISettings settings = new();
    private SendMotion? sendMotion;

    public ObservableCollection<ConversationRow> Summaries { get; } = new();
    public string NewConversationLabel => L10n.ActionNewConversation();
    public string SearchLabel => L10n.ConversationSearchPlaceholder();
    public string HistoryLabel => L10n.LabelConversations();
    public string HistoryEmptyLabel => L10n.ChatHistoryEmpty();
    public string TranscriptLabel => L10n.LabelTranscript();
    public string EmptyTitle => L10n.ChatEmptyTitle();
    public string EmptyHint => L10n.ChatEmptyHint();
    public string ComposerLabel => L10n.ComposerPlaceholder();
    public string SendLabel => L10n.ActionSend();
    public string AppName => L10n.AppName();
    public string NewConversationTooltip => L10n.ActionNewConversationShortcut("Ctrl+N");
    public string ComposerHint => L10n.ComposerHintMultiline();
    public string LatestLabel => L10n.ActionScrollToLatest();
    public TitleBar WindowTitleBar => AppTitleBar;

    public ChatView(ProductSessionHandle session)
    {
        this.session = session;
        list = session.Conversations();
        InitializeComponent();
        current = Create(session.Chat());
        DataContext = current;
        history = new(
            list.State,
            list.ListChanges,
            action => DispatcherQueue.TryEnqueue(() => action()),
            EqualityComparer<ChatSummary[]>.Create(
                (left, right) => left.AsSpan().SequenceEqual(right)
            )
        );
        history.Changed += QueueHistoryRefresh;
        RefreshHistory();
        if (OperatingSystem.IsWindowsVersionAtLeast(10, 0, 19041))
            settings.AnimationsEnabledChanged += AnimationsEnabledChanged;
        Composer.TextCompositionStarted += (_, _) => composing = true;
        Composer.TextCompositionEnded += (_, _) => composing = false;
        Transcript.ContainerContentChanging += (_, args) =>
        {
            if (
                sendMotion?.Destination is { } destination
                && args.InRecycleQueue
                && args.Item == destination
            )
                CancelSendMotion();
        };
        Loaded += (_, _) => Composer.Focus(FocusState.Programmatic);
    }

    private ConversationModel Create(ChatHandle handle)
    {
        var model = new ConversationModel(handle, DispatcherQueue);
        conversations.Add(model);
        model.PropertyChanged += (_, args) =>
        {
            if (
                model == current
                && !disposed
                && args.PropertyName is nameof(model.IsEmpty) or nameof(model.IsSending)
            )
                UpdatePresentation();
        };
        model.Messages.CollectionChanged += (_, change) =>
        {
            if (model != current || disposed)
                return;
            if (sendMotion is { Destination: null, HasStarted: false } motion)
                motion.Destination = change
                    .NewItems?.OfType<MessageRow>()
                    .FirstOrDefault(row => row.IsOutgoing);
            QueueScroll();
            if (restoringPosition || change.NewItems is null)
                return;
            foreach (MessageRow row in change.NewItems)
                if (!row.IsOutgoing)
                    FrameworkElementAutomationPeer
                        .CreatePeerForElement(Transcript)
                        ?.RaiseNotificationEvent(
                            AutomationNotificationKind.Other,
                            AutomationNotificationProcessing.CurrentThenMostRecent,
                            $"{row.Author}: {row.Text}",
                            "IncomingMessage"
                        );
        };
        return model;
    }

    private void Bind(ConversationModel model)
    {
        if (model == current)
            return;
        SaveReadingPosition();
        CancelSendMotion();
        restoringPosition = true;
        realizingReadingAnchor = false;
        restoringOffset = null;
        current.SetActive(false);
        current = model;
        current.SetActive(true);
        UpdateScrollMode();
        DataContext = current;
        UpdatePresentation();
        RefreshHistory();
        QueueScroll();
    }

    private void UpdatePresentation()
    {
        EmptyState.Visibility = current.IsEmpty ? Visibility.Visible : Visibility.Collapsed;
        SendingRing.IsActive = current.IsSending;
        SendingRing.Visibility = current.IsSending ? Visibility.Visible : Visibility.Collapsed;
        SendGlyph.Visibility = current.IsSending ? Visibility.Collapsed : Visibility.Visible;
    }

    private async void Send(object sender, RoutedEventArgs args) => await SendCurrent();

    private async Task SendCurrent()
    {
        var model = current;
        if (!model.CanSend)
            return;
        CancelSendMotion();
        if (
            settings.AnimationsEnabled
            && ComposerCard.IsLoaded
            && Composer.FindDescendant<ScrollViewer>() is { ScrollableHeight: <= 1 }
        )
        {
            ComposerCard.UpdateLayout();
            model.HoldReplies();
            sendMotion = new(
                SendLayer,
                ComposerCard,
                Composer,
                motion =>
                {
                    if (sendMotion == motion)
                        sendMotion = null;
                    model.ReleaseReplies(
                        motion.Completed && model == current && settings.AnimationsEnabled
                    );
                }
            );
        }
        model.FollowLatest = true;
        Composer.Focus(FocusState.Programmatic);
        await model.SendAsync();
        if (disposed || model != current)
            return;
        if (model.HasError)
            CancelSendMotion();
        history.RefreshNow();
        QueueScroll();
    }

    private void NewChat(XamlUICommand sender, ExecuteRequestedEventArgs args)
    {
        var pending = conversations.FirstOrDefault(model => model.Id is null);
        Bind(pending ?? Create(session.NewChat()));
        FocusComposer();
    }

    private void SelectChat(object sender, SelectionChangedEventArgs args)
    {
        if (
            updatingHistory
            || History.SelectedItem is not ConversationRow summary
            || summary.Id == current.Id
        )
            return;
        OpenConversation(summary);
    }

    private void ActivateChat(object sender, ItemClickEventArgs args)
    {
        OpenConversation((ConversationRow)args.ClickedItem);
        FocusComposer();
    }

    private void OpenConversation(ConversationRow summary)
    {
        var cached = conversations.FirstOrDefault(model => model.Id == summary.Id);
        if (cached is not null)
            Bind(cached);
        else if (session.SelectChat(summary.Id) is { } handle)
            Bind(Create(handle));
    }

    private void CopyMessage(XamlUICommand sender, ExecuteRequestedEventArgs args)
    {
        if (MessageMenu.Target is not ListViewItem { Content: MessageRow message })
            return;
        var data = new DataPackage();
        data.SetText(message.Text);
        Clipboard.SetContent(data);
    }

    private void SearchChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
    {
        if (args.Reason == AutoSuggestionBoxTextChangeReason.UserInput)
            RefreshHistory();
    }

    private void TitleBarToggle(TitleBar sender, object args) => ToggleHistory();

    private void ToggleHistory()
    {
        Shell.IsPaneOpen = !Shell.IsPaneOpen;
        if (Shell.IsPaneOpen)
            History.Focus(FocusState.Programmatic);
        else
            Composer.Focus(FocusState.Programmatic);
    }

    private void FocusComposer()
    {
        if (Shell.DisplayMode == SplitViewDisplayMode.Overlay)
            Shell.IsPaneOpen = false;
        Composer.Focus(FocusState.Programmatic);
    }

    private void FocusSearch(XamlUICommand sender, ExecuteRequestedEventArgs args)
    {
        Shell.IsPaneOpen = true;
        Search.Focus(FocusState.Programmatic);
    }

    private void ToggleHistoryShortcut(
        KeyboardAccelerator sender,
        KeyboardAcceleratorInvokedEventArgs args
    )
    {
        ToggleHistory();
        args.Handled = true;
    }

    private void FocusComposerShortcut(
        KeyboardAccelerator sender,
        KeyboardAcceleratorInvokedEventArgs args
    )
    {
        FocusComposer();
        args.Handled = true;
    }

    private async void SendShortcut(
        KeyboardAccelerator sender,
        KeyboardAcceleratorInvokedEventArgs args
    )
    {
        if (composing)
            return;
        args.Handled = true;
        await SendCurrent();
    }

    private async void ComposerKeyDown(object sender, KeyRoutedEventArgs args)
    {
        if (
            args.Key != VirtualKey.Enter
            || composing
            || IsDown(VirtualKey.Shift)
            || IsDown(VirtualKey.Control)
            || IsDown(VirtualKey.Menu)
        )
            return;
        args.Handled = true;
        await SendCurrent();
    }

    private static bool IsDown(VirtualKey key) =>
        InputKeyboardSource.GetKeyStateForCurrentThread(key).HasFlag(CoreVirtualKeyStates.Down);

    private void ComposerFocusChanged(object sender, RoutedEventArgs args) =>
        VisualStateManager.GoToState(
            this,
            Composer.FocusState == FocusState.Unfocused ? "ComposerUnfocused" : "ComposerFocused",
            false
        );

    private void AnimationsEnabledChanged(
        UISettings sender,
        UISettingsAnimationsEnabledChangedEventArgs args
    )
    {
        DispatcherQueue.TryEnqueue(() =>
        {
            if (disposed || settings.AnimationsEnabled)
                return;
            CancelSendMotion();
            foreach (var model in conversations)
                model.StopReplyMotion();
            foreach (var bubble in Transcript.FindDescendants().OfType<MessageBubble>())
                bubble.StopReveal();
        });
    }

    private void CancelSendMotion()
    {
        sendMotion?.Dispose();
        sendMotion = null;
    }

    public async ValueTask DisposeAsync()
    {
        if (disposed)
            return;
        disposed = true;
        if (OperatingSystem.IsWindowsVersionAtLeast(10, 0, 19041))
            settings.AnimationsEnabledChanged -= AnimationsEnabledChanged;
        CancelSendMotion();
        history.Dispose();
        await Task.WhenAll(conversations.Select(model => model.DisposeAsync().AsTask()));
        list.Dispose();
    }
}
