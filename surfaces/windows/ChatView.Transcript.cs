using CommunityToolkit.WinUI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Arut.Surface.Windows;

public sealed partial class ChatView
{
    private const double TranscriptClearance = 16;
    private bool scrollPending;
    private bool scrollDirty;
    private bool restoringPosition;
    private bool realizingReadingAnchor;
    private double? restoringOffset;
    private ScrollViewer? transcriptScroll;
    private double motionScrollOffset;

    private void TranscriptLoaded(object sender, RoutedEventArgs args)
    {
        if (transcriptScroll is not null)
            return;
        transcriptScroll = Transcript.FindDescendant<ScrollViewer>();
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
        if (IsLoaded)
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
        if (IsLoaded)
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
                if (
                    current.FollowLatest
                    && current.Messages.LastOrDefault() is { } last
                    && Transcript.ContainerFromItem(last) is null
                )
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
                        && container.FindDescendant<MessageBubble>() is { } bubble
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
        FrameworkElement? first = null;
        var firstTop = double.PositiveInfinity;
        foreach (var child in panel.Children)
        {
            if (child is not FrameworkElement element)
                continue;
            var top =
                element
                    .TransformToVisual((UIElement)transcriptScroll.Content)
                    .TransformPoint(new())
                    .Y - transcriptScroll.VerticalOffset;
            if (top + element.ActualHeight > 0 && top < Transcript.ActualHeight && top < firstTop)
            {
                first = element;
                firstTop = top;
            }
        }
        if (first is not null && Transcript.ItemFromContainer(first) is MessageRow row)
        {
            current.ReadingAnchor = row;
            current.ReadingAnchorOffset = firstTop;
        }
    }

    private void ScrollToLatest(object sender, RoutedEventArgs args)
    {
        Composer.Focus(FocusState.Programmatic);
        current.FollowLatest = true;
        QueueScroll();
    }
}
