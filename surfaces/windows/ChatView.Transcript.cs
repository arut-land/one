using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Arut.Surface.Windows;

/// <summary>
/// Tail-following and the insets the floating composer needs. Scroll anchoring
/// itself belongs to `ItemsStackPanel.ItemsUpdatingScrollMode`, not to us.
/// </summary>
public sealed partial class ChatView
{
    private const double TranscriptClearance = 16;
    private ScrollViewer? transcriptScroll;
    private bool followLatest = true;
    private bool scrollPending;
    private bool animateNextScroll;

    private void TranscriptLoaded(object sender, RoutedEventArgs args)
    {
        if (transcriptScroll is not null)
            return;
        var scroll = FindDescendant<ScrollViewer>(Transcript);
        transcriptScroll = scroll;
        if (scroll is null)
            return;
        scroll.DirectManipulationStarted += (_, _) => CancelSendTransition();
        scroll.AddHandler(
            UIElement.PointerWheelChangedEvent,
            new PointerEventHandler((_, _) => CancelSendTransition()),
            true
        );
        scroll.ViewChanged += (_, change) =>
        {
            if (disposed || change.IsIntermediate || scrollPending)
                return;
            followLatest = scroll.ScrollableHeight - scroll.VerticalOffset < 48;
            UpdateScrollMode();
            UpdateAffordances();
        };
        QueueScroll();
    }

    private void ComposerAreaSizeChanged(object sender, SizeChangedEventArgs args)
    {
        if (args.PreviousSize.Height == args.NewSize.Height)
            return;
        // A scrolling footer keeps the final message above the floating editor.
        // The viewport itself extends behind it, including its native scrollbar.
        TranscriptEndSpace.Height = args.NewSize.Height + TranscriptClearance;
        LatestArea.Margin = new(24, 0, 24, args.NewSize.Height + 8);
        EmptyState.Margin = new(24, 24, 24, args.NewSize.Height + 24);
        if (IsLoaded)
            QueueScroll();
    }

    private void ScrollToLatest(object sender, RoutedEventArgs args)
    {
        CancelSendTransition();
        Composer.Focus(FocusState.Keyboard);
        followLatest = true;
        animateNextScroll = motionSettings.AnimationsEnabled;
        UpdateAffordances();
        QueueScroll();
    }

    private void QueueScroll()
    {
        if (disposed || scrollPending)
            return;
        scrollPending = DispatcherQueue.TryEnqueue(() =>
        {
            var animate = animateNextScroll;
            animateNextScroll = false;
            try
            {
                if (disposed || transcriptScroll is not { } scroll)
                    return;
                UpdateScrollMode();
                if (followLatest && !animate && ViewModel.Conversation.Messages.LastOrDefault() is { } last)
                    Transcript.ScrollIntoView(last);
                // The footer and realized rows determine the final extent.
                // Reading ScrollableHeight before this layout can leave a new
                // reply behind the composer and be mistaken for user scrolling.
                Transcript.UpdateLayout();
                if (followLatest)
                    scroll.ChangeView(null, scroll.ScrollableHeight, null, !animate);
                StartMessageMotion();
                UpdateAffordances();
            }
            finally
            {
                scrollPending = false;
            }
        });
    }

    private void UpdateAffordances()
    {
        var distance = transcriptScroll is { } scroll ? scroll.ScrollableHeight - scroll.VerticalOffset : 0;
        // Hysteresis prevents the affordance flickering around the tail. A
        // small scroll adjustment does not need another control on screen.
        var threshold = LatestSurface.Visibility == Visibility.Visible ? 48 : 120;
        LatestSurface.Visibility =
            !ViewModel.Conversation.IsEmpty && !followLatest
                && sendTransition is null && distance > threshold
                ? Visibility.Visible
                : Visibility.Collapsed;
    }

    private void UpdateScrollMode()
    {
        if (Transcript.ItemsPanelRoot is ItemsStackPanel panel)
            panel.ItemsUpdatingScrollMode = followLatest
                ? ItemsUpdatingScrollMode.KeepLastItemInView
                : ItemsUpdatingScrollMode.KeepItemsInView;
    }

    private static T? FindDescendant<T>(DependencyObject root)
        where T : DependencyObject
    {
        var count = VisualTreeHelper.GetChildrenCount(root);
        for (var index = 0; index < count; index++)
        {
            var child = VisualTreeHelper.GetChild(root, index);
            if (child is T match)
                return match;
            if (FindDescendant<T>(child) is { } nested)
                return nested;
        }
        return null;
    }
}
