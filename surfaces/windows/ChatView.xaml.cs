using System.Collections.ObjectModel;
using Arut.Bindings;
using global::Windows.System;
using global::Windows.UI.Core;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Arut.Surface.Windows;

public sealed partial class ChatView : UserControl, IAsyncDisposable
{
    private const double TranscriptClearance = 16;
    private readonly ProductSessionHandle session;
    private readonly ConversationsHandle list;
    private readonly ObservableState<ChatSummary[]> history;
    private readonly List<ConversationModel> conversations = new();
    private ConversationModel current;
    private bool updatingHistory;
    private bool disposed;
    private bool composing;
    private bool scrollPending;
    private bool scrollDirty;
    private bool restoringPosition;
    private bool realizingReadingAnchor;
    private double? restoringOffset;
    private ScrollViewer? transcriptScroll;
    private readonly TextBlock composerMeasure = new() { TextWrapping = TextWrapping.Wrap };
    private readonly global::Windows.UI.ViewManagement.UISettings settings = new();
    private SendMotion? sendMotion;
    private double motionScrollOffset;

    public ObservableCollection<ConversationRow> Summaries { get; } = new();
    public string NewConversationLabel => L10n.ActionNewConversation();
    public string SearchLabel => L10n.ConversationSearchPlaceholder();
    public string HistoryLabel => L10n.LabelConversations();
    public string HistoryEmptyLabel => L10n.ChatHistoryEmpty();
    public string SessionLabel => L10n.ChatLocalSession();
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
            action => DispatcherQueue.TryEnqueue(() => action())
        );
        history.Changed += RefreshHistory;
        RefreshHistory();
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
        Composer.TextChanged += (_, _) => SizeComposer();
        Composer.SizeChanged += (_, args) =>
        {
            if (args.PreviousSize.Width != args.NewSize.Width)
                SizeComposer();
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
            if (model == current && !disposed)
            {
                if (
                    sendMotion is { Destination: null, HasStarted: false } motion
                    && change.NewItems is not null
                )
                    foreach (MessageRow row in change.NewItems)
                        if (row.IsOutgoing)
                        {
                            motion.Destination = row;
                            break;
                        }
                QueueScroll();
                RefreshHistory();
            }
        };
        return model;
    }

    private void Bind(ConversationModel model)
    {
        if (model == current)
        {
            if (Shell.DisplayMode == SplitViewDisplayMode.Overlay)
                Shell.IsPaneOpen = false;
            Composer.Focus(FocusState.Programmatic);
            return;
        }
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
        if (Shell.DisplayMode == SplitViewDisplayMode.Overlay)
            Shell.IsPaneOpen = false;
        Composer.Focus(FocusState.Programmatic);
    }

    private void RefreshHistory()
    {
        if (disposed)
            return;
        updatingHistory = true;
        try
        {
            var rows = history
                .Value.Where(row =>
                    row.Title.Contains(Search.Text, StringComparison.CurrentCultureIgnoreCase)
                )
                .ToArray();
            for (var i = 0; i < rows.Length; i++)
            {
                if (i < Summaries.Count && Summaries[i].Id == rows[i].Id)
                {
                    var preview = Preview(rows[i].Id);
                    if (Summaries[i].Title != rows[i].Title || Summaries[i].Preview != preview)
                        Summaries[i] = new(rows[i], preview);
                }
                else
                    Summaries.Insert(i, new(rows[i], Preview(rows[i].Id)));
            }
            while (Summaries.Count > rows.Length)
                Summaries.RemoveAt(Summaries.Count - 1);
            History.SelectedItem = Summaries.FirstOrDefault(row => row.Id == current.Id);
            HistoryEmpty.Text =
                Search.Text.Length == 0 ? HistoryEmptyLabel : L10n.ConversationSearchEmpty();
            HistoryEmpty.Visibility =
                Summaries.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
            ConversationTitle.Text =
                history.Value.FirstOrDefault(row => row.Id == current.Id).Title
                ?? NewConversationLabel;
        }
        finally
        {
            updatingHistory = false;
        }
    }

    private string Preview(string id) =>
        conversations
            .FirstOrDefault(model => model.Id == id)
            ?.Messages.LastOrDefault()
            ?.Text.ReplaceLineEndings(" ")
        ?? "";

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
            && Descendant<ScrollViewer>(Composer) is { ScrollableHeight: <= 1 }
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
                    {
                        sendMotion = null;
                    }
                    model.ReleaseReplies(
                        motion.Completed && model == current && settings.AnimationsEnabled
                    );
                }
            );
        }
        model.FollowLatest = true;
        await model.SendAsync();
        if (disposed || model != current)
            return;
        if (model.HasError)
            CancelSendMotion();
        history.RefreshNow();
        RefreshHistory();
        QueueScroll();
        Composer.Focus(FocusState.Programmatic);
    }

    private void NewChat(object sender, RoutedEventArgs args)
    {
        var pending = conversations.FirstOrDefault(model => model.Id is null);
        Bind(pending ?? Create(session.NewChat()));
    }

    private void SelectChat(object sender, SelectionChangedEventArgs args)
    {
        if (
            updatingHistory
            || History.SelectedItem is not ConversationRow summary
            || summary.Id == current.Id
        )
            return;
        var cached = conversations.FirstOrDefault(model => model.Id == summary.Id);
        if (cached is not null)
            Bind(cached);
        else if (session.SelectChat(summary.Id) is { } handle)
            Bind(Create(handle));
    }

    private void SearchChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
    {
        if (history is not null)
            RefreshHistory();
    }

    private void TitleBarToggle(TitleBar sender, object args) =>
        Shell.IsPaneOpen = !Shell.IsPaneOpen;

    private void FocusSearch(object sender, RoutedEventArgs args)
    {
        Shell.IsPaneOpen = true;
        Search.Focus(FocusState.Keyboard);
    }

    private void SearchShortcut(
        KeyboardAccelerator sender,
        KeyboardAcceleratorInvokedEventArgs args
    )
    {
        FocusSearch(this, new());
        args.Handled = true;
    }

    private void NewConversationShortcut(
        KeyboardAccelerator sender,
        KeyboardAcceleratorInvokedEventArgs args
    )
    {
        NewChat(this, new());
        args.Handled = true;
    }

    private void ToggleHistoryShortcut(
        KeyboardAccelerator sender,
        KeyboardAcceleratorInvokedEventArgs args
    )
    {
        Shell.IsPaneOpen = !Shell.IsPaneOpen;
        args.Handled = true;
    }

    private void FocusComposerShortcut(
        KeyboardAccelerator sender,
        KeyboardAcceleratorInvokedEventArgs args
    )
    {
        Composer.Focus(FocusState.Keyboard);
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

    private void SizeComposer()
    {
        var width = Composer.ActualWidth - Composer.Padding.Left - Composer.Padding.Right;
        if (width <= 0)
            return;
        composerMeasure.FontFamily = Composer.FontFamily;
        composerMeasure.FontSize = Composer.FontSize;
        composerMeasure.Text = Composer.Text + "\u200b";
        composerMeasure.Measure(
            new global::Windows.Foundation.Size(width, double.PositiveInfinity)
        );
        Composer.Height = Math.Clamp(
            Math.Ceiling(
                composerMeasure.DesiredSize.Height + Composer.Padding.Top + Composer.Padding.Bottom
            ),
            36,
            164
        );
    }

    private void ComposerFocusChanged(object sender, RoutedEventArgs args) =>
        VisualStateManager.GoToState(
            this,
            Composer.FocusState == FocusState.Unfocused ? "ComposerUnfocused" : "ComposerFocused",
            false
        );

    private static T? Descendant<T>(DependencyObject parent)
        where T : DependencyObject
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(parent); i++)
        {
            var child = VisualTreeHelper.GetChild(parent, i);
            if (child is T match)
                return match;
            if (Descendant<T>(child) is { } nested)
                return nested;
        }
        return null;
    }

    private void TranscriptLoaded(object sender, RoutedEventArgs args)
    {
        if (transcriptScroll is not null)
            return;
        transcriptScroll = Descendant<ScrollViewer>(Transcript);
        if (transcriptScroll is null)
            return;
        transcriptScroll.ViewChanged += (_, change) =>
        {
            if (disposed)
                return;
            UpdateScrollAffordances();
            if (restoringPosition)
            {
                if (change.IsIntermediate)
                    return;
                if (realizingReadingAnchor)
                {
                    realizingReadingAnchor = false;
                    RestoreReadingPosition();
                }
                else if (
                    restoringOffset is { } offset
                    && Math.Abs(transcriptScroll.VerticalOffset - offset) < 1
                )
                {
                    restoringPosition = false;
                    restoringOffset = null;
                }
                return;
            }
            if (scrollPending)
                return;
            var movedDuringSend =
                sendMotion is { HasStarted: true }
                && Math.Abs(transcriptScroll.VerticalOffset - motionScrollOffset) > 1;
            if (change.IsIntermediate && !movedDuringSend)
                return;
            // Record the user's position before cancellation releases replies and
            // queues another layout pass. That pass must respect the new position.
            current.FollowLatest =
                transcriptScroll.ScrollableHeight - transcriptScroll.VerticalOffset < 48;
            SaveReadingPosition();
            UpdateScrollMode();
            UpdateScrollAffordances();
            if (movedDuringSend)
                CancelSendMotion();
        };
        QueueScroll();
    }

    private void TranscriptSizeChanged(object sender, SizeChangedEventArgs args)
    {
        if (
            args.PreviousSize.Width != args.NewSize.Width
            || (
                args.PreviousSize.Height != args.NewSize.Height
                && sendMotion is { HasStarted: true }
            )
        )
            CancelSendMotion();
        if (current is not null)
            QueueScroll();
    }

    private void ComposerAreaSizeChanged(object sender, SizeChangedEventArgs args)
    {
        if (args.PreviousSize.Height == args.NewSize.Height)
            return;
        if (sendMotion is { HasStarted: true })
            CancelSendMotion();
        // A scrolling footer keeps the final message above the floating editor.
        // The viewport itself extends behind it, including its native scrollbar.
        TranscriptEndSpace.Height = args.NewSize.Height + TranscriptClearance;
        BottomScrollFade.Height = args.NewSize.Height + 40;
        LatestArea.Margin = new(24, 0, 24, args.NewSize.Height + 8);
        EmptyState.Margin = new(24, 24, 24, args.NewSize.Height + 24);
        if (current is not null)
            QueueScroll();
    }

    private void QueueScroll()
    {
        if (disposed)
            return;
        scrollDirty = true;
        if (scrollPending)
            return;
        scrollPending = DispatcherQueue.TryEnqueue(
            Microsoft.UI.Dispatching.DispatcherQueuePriority.Normal,
            () =>
            {
                scrollDirty = false;
                if (disposed || transcriptScroll is null)
                {
                    scrollPending = false;
                    return;
                }
                Transcript.UpdateLayout();
                UpdateScrollMode();
                if (current.FollowLatest && current.Messages.LastOrDefault() is { } last)
                {
                    Transcript.ScrollIntoView(last);
                    Transcript.UpdateLayout();
                }
                if (current.FollowLatest)
                    transcriptScroll.ChangeView(
                        null,
                        transcriptScroll.ScrollableHeight,
                        null,
                        true
                    );
                else if (restoringPosition && !realizingReadingAnchor && restoringOffset is null)
                    RestoreReadingPosition();
                if (current.FollowLatest)
                    restoringPosition = false;
                if (sendMotion is { HasStarted: false, Destination: { } destination })
                {
                    // Scroll/layout first, then resolve the exact item. The nested
                    // UserControl has its own namescope, so use its named elements.
                    if (
                        Transcript.ContainerFromItem(destination) is { } container
                        && Descendant<MessageBubble>(container) is { } bubble
                        && bubble.Message == destination
                        && bubble.CanAnimateWithin(Transcript, ComposerArea.ActualHeight)
                    )
                    {
                        motionScrollOffset = transcriptScroll.VerticalOffset;
                        _ = sendMotion.StartAsync(bubble);
                    }
                    else
                        CancelSendMotion();
                }
                scrollPending = false;
                UpdateScrollAffordances();
                if (scrollDirty)
                    QueueScroll();
            }
        );
    }

    private void UpdateScrollAffordances()
    {
        var hasMessages = !current.IsEmpty;
        TopScrollFade.Visibility =
            hasMessages && transcriptScroll?.VerticalOffset > 1
                ? Visibility.Visible
                : Visibility.Collapsed;
        BottomScrollFade.Visibility =
            hasMessages
            && transcriptScroll is { } scroll
            && scroll.ScrollableHeight - scroll.VerticalOffset > TranscriptClearance
                ? Visibility.Visible
                : Visibility.Collapsed;
        LatestSurface.Visibility =
            hasMessages && !current.FollowLatest ? Visibility.Visible : Visibility.Collapsed;
    }

    private global::Windows.UI.Color TransparentColor(global::Windows.UI.Color color)
    {
        // Preserve the theme RGB while fading alpha; Transparent interpolates white.
        color.A = 0;
        return color;
    }

    private void RestoreReadingPosition()
    {
        if (disposed || transcriptScroll is null)
            return;
        double offset = 0;
        if (!current.ReadingAtTop && current.ReadingAnchor is { } anchor)
        {
            if (Transcript.ContainerFromItem(anchor) is not FrameworkElement container)
            {
                // Native realization must finish before applying the local offset.
                realizingReadingAnchor = true;
                Transcript.ScrollIntoView(anchor, ScrollIntoViewAlignment.Leading);
                return;
            }
            offset =
                container
                    .TransformToVisual((UIElement)transcriptScroll.Content)
                    .TransformPoint(new())
                    .Y - current.ReadingAnchorOffset;
        }
        restoringOffset = Math.Clamp(offset, 0, transcriptScroll.ScrollableHeight);
        if (!transcriptScroll.ChangeView(null, restoringOffset, null, true))
        {
            restoringPosition = false;
            restoringOffset = null;
        }
    }

    private void UpdateScrollMode()
    {
        if (Transcript.ItemsPanelRoot is ItemsStackPanel panel)
            panel.ItemsUpdatingScrollMode = current.FollowLatest
                ? ItemsUpdatingScrollMode.KeepLastItemInView
                : ItemsUpdatingScrollMode.KeepItemsInView;
    }

    private void SaveReadingPosition()
    {
        if (
            restoringPosition
            || current.FollowLatest
            || transcriptScroll is null
            || Transcript.ItemsPanelRoot is not ItemsStackPanel panel
        )
            return;
        current.ReadingAtTop = transcriptScroll.VerticalOffset < 1;
        if (current.ReadingAtTop)
        {
            current.ReadingAnchor = null;
            current.ReadingAnchorOffset = 0;
            return;
        }
        // Virtualized rows have estimated heights. Preserve the first visible
        // message and its local offset instead of a global estimated scroll offset.
        var first = panel
            .Children.OfType<FrameworkElement>()
            .Select(element =>
                (
                    Element: element,
                    Top: element
                        .TransformToVisual((UIElement)transcriptScroll.Content)
                        .TransformPoint(new())
                        .Y - transcriptScroll.VerticalOffset
                )
            )
            .Where(item =>
                item.Top + item.Element.ActualHeight > 0 && item.Top < Transcript.ActualHeight
            )
            .OrderBy(item => item.Top)
            .FirstOrDefault();
        if (
            first.Element is not null
            && Transcript.ItemFromContainer(first.Element) is MessageRow row
        )
        {
            current.ReadingAnchor = row;
            current.ReadingAnchorOffset = first.Top;
        }
    }

    private void ScrollToLatest(object sender, RoutedEventArgs args)
    {
        current.FollowLatest = true;
        QueueScroll();
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
        CancelSendMotion();
        history.Dispose();
        await Task.WhenAll(conversations.Select(async model => await model.DisposeAsync()));
        list.Dispose();
    }
}

// WinUI's XAML compiler generates setters for record structs. Expose read-only
// presentation properties without changing the generated Rust value types.
public sealed class ConversationRow(ChatSummary summary, string preview)
{
    public string Id => summary.Id;
    public string Title => summary.Title;
    public string Preview => preview;

    public override string ToString() => Title;
}
