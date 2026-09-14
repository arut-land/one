using System.Numerics;
using global::Windows.UI.ViewManagement;
using Microsoft.UI.Composition;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media.Animation;

namespace Arut.Surface.Windows;

public sealed partial class ChatView
{
    private const int FastMotion = 167;
    private const int FeedbackMotion = 83;
    private readonly UISettings motionSettings = new();
    private readonly List<MessageRow> arrivingMessages = [];
    private DispatcherQueueTimer? sendExpiry;
    private SendTransition? sendTransition;

    // Only presentation state: the core's accepted row remains the source of
    // truth. Neither replies nor editing wait for an animation to complete.
    private sealed class SendTransition(
        ConversationViewModel owner,
        string text,
        ulong afterId,
        TextBlock snapshot,
        Border sourceSurface,
        ConnectedAnimation surface,
        ConnectedAnimation content
    )
    {
        public ConversationViewModel Owner { get; } = owner;
        public string Text { get; } = text;
        public ulong AfterId { get; } = afterId;
        public TextBlock Snapshot { get; } = snapshot;
        public Border SourceSurface { get; } = sourceSurface;
        public ConnectedAnimation Surface { get; } = surface;
        public ConnectedAnimation Content { get; } = content;
        public MessageRow? Destination { get; set; }
        public bool Started { get; set; }
        public int CompletedParts { get; set; }
        public UIElement? Timestamp { get; set; }
    }

    private void InitializeMotion()
    {
        if (OperatingSystem.IsWindowsVersionAtLeast(10, 0, 19041))
            motionSettings.AnimationsEnabledChanged += MotionSettingsChanged;
        Loaded += MotionLoaded;
        SizeChanged += MotionSizeChanged;
        Shell.PaneOpening += PaneOpening;
        Shell.PaneClosing += PaneClosing;
        Transcript.ContainerContentChanging += TranscriptContainerChanging;
    }

    private void MotionLoaded(object sender, RoutedEventArgs args)
    {
        ConfigureVisibilityMotion();
        // Initialize the native service when the view is ready, before the
        // first send. Fast-out motion avoids a slow, barely visible onset.
        var service = ConnectedAnimationService.GetForCurrentView();
        service.DefaultDuration = TimeSpan.FromMilliseconds(FastMotion);
        service.DefaultEasingFunction = ElementCompositionPreview.GetElementVisual(this).Compositor
            .CreateCubicBezierEasingFunction(Vector2.Zero, new(0, 1));
    }

    private void ConfigureVisibilityMotion()
    {
        SendButton.MotionEnabled = motionSettings.AnimationsEnabled;
        LatestSurface.MotionEnabled = motionSettings.AnimationsEnabled;
        foreach (var element in new UIElement[] { LatestSurface, EmptyState })
        {
            var visual = ElementCompositionPreview.GetElementVisual(element);
            ElementCompositionPreview.SetImplicitShowAnimation(
                element,
                motionSettings.AnimationsEnabled ? Fade(visual.Compositor, 0, 1, FastMotion) : null
            );
            ElementCompositionPreview.SetImplicitHideAnimation(
                element,
                motionSettings.AnimationsEnabled ? Fade(visual.Compositor, 1, 0, FeedbackMotion) : null
            );
            if (!motionSettings.AnimationsEnabled)
            {
                visual.StopAnimation("Opacity");
                visual.Opacity = 1;
            }
        }
    }

    private static ScalarKeyFrameAnimation Fade(Compositor compositor, float from, float to, int ms)
    {
        var animation = compositor.CreateScalarKeyFrameAnimation();
        animation.Target = "Opacity";
        animation.InsertKeyFrame(0, from);
        var easing = to > from
            ? compositor.CreateCubicBezierEasingFunction(Vector2.Zero, new(0, 1))
            : compositor.CreateCubicBezierEasingFunction(new(1, 0), Vector2.One);
        animation.InsertKeyFrame(1, to, easing);
        animation.Duration = TimeSpan.FromMilliseconds(ms);
        return animation;
    }

    private void PaneOpening(SplitView sender, object args)
    {
        CancelSendTransition();
        if (!motionSettings.AnimationsEnabled)
            return;
        // SplitView already animates pane and content translation. Only soften
        // the history content's entrance; do not stack another slide over it.
        var visual = ElementCompositionPreview.GetElementVisual(HistoryPane);
        visual.StartAnimation("Opacity", Fade(visual.Compositor, 0, 1, FastMotion));
    }

    private void PaneClosing(SplitView sender, SplitViewPaneClosingEventArgs args) =>
        CancelSendTransition();

    private void PrepareSendTransition()
    {
        CancelSendTransition();
        followLatest = true;
        animateNextScroll = false;
        Composer.Focus(FocusState.Keyboard);
        if (!motionSettings.AnimationsEnabled || !Composer.IsLoaded)
            return;
        // A scrolled editor has only part of the message on screen. Let the
        // accepted row appear normally instead of flying a clipped fragment.
        var editor = FindDescendant<ScrollViewer>(Composer);
        if (editor is null || editor.ScrollableHeight > 1)
            return;
        var service = ConnectedAnimationService.GetForCurrentView();
        var sourceSurface = new Border
        {
            Width = ComposerSurface.ActualWidth,
            Height = ComposerSurface.ActualHeight,
            Background = ComposerSurface.Background,
            BorderBrush = ComposerSurface.BorderBrush,
            BorderThickness = ComposerSurface.BorderThickness,
            CornerRadius = ComposerSurface.CornerRadius,
            IsHitTestVisible = false,
        };
        AutomationProperties.SetAccessibilityView(sourceSurface, AccessibilityView.Raw);
        SendCaptureLayer.Children.Add(sourceSurface);
        // Capture a native text element, not the editor's caret or selection.
        // Keep its visual alive through cancellation and completion; the live
        // TextBox remains a separate, usable control throughout the transfer.
        var owner = ViewModel.Conversation;
        var origin = editor.TransformToVisual(SendCaptureLayer).TransformPoint(new());
        var snapshot = new TextBlock
        {
            Text = owner.Draft.Text.Trim(),
            FontFamily = Composer.FontFamily,
            FontSize = Composer.FontSize,
            FontWeight = Composer.FontWeight,
            Foreground = Composer.Foreground,
            TextWrapping = TextWrapping.Wrap,
            Width = editor.ActualWidth,
            IsHitTestVisible = false,
        };
        AutomationProperties.SetAccessibilityView(snapshot, AccessibilityView.Raw);
        Canvas.SetLeft(snapshot, origin.X);
        Canvas.SetTop(snapshot, origin.Y);
        SendCaptureLayer.Children.Add(snapshot);
        SendCaptureLayer.UpdateLayout();
        var surface = service.PrepareToAnimate("SendSurface", sourceSurface);
        var content = service.PrepareToAnimate("SendText", snapshot);
        surface.Configuration = new BasicConnectedAnimationConfiguration();
        content.Configuration = new BasicConnectedAnimationConfiguration();
        content.IsScaleAnimationEnabled = false;
        sendTransition = new(
            owner,
            owner.Draft.Text.Trim(),
            owner.Messages.LastOrDefault()?.Id ?? 0,
            snapshot,
            sourceSurface,
            surface,
            content
        );
        surface.Completed += SendTransitionCompleted;
        content.Completed += SendTransitionCompleted;
        sendExpiry ??= CreateSendExpiry();
        sendExpiry.Start();
    }

    private DispatcherQueueTimer CreateSendExpiry()
    {
        var timer = DispatcherQueue.CreateTimer();
        timer.Interval = TimeSpan.FromSeconds(1);
        timer.IsRepeating = false;
        timer.Tick += (_, _) => CancelSendTransition();
        return timer;
    }

    private void ObserveMessageMotion(MessageRow row)
    {
        if (!motionSettings.AnimationsEnabled)
        {
            if (!row.IsOutgoing)
                AnnounceMessage(row);
            return;
        }
        if (sendTransition is { Started: false, Destination: null } transition
            && transition.Owner == ViewModel.Conversation
            && row.IsOutgoing && row.Id > transition.AfterId && row.Text == transition.Text)
            transition.Destination = row;
        else if (!row.IsOutgoing)
        {
            if (followLatest)
                arrivingMessages.Add(row);
            else
                AnnounceMessage(row);
        }
    }

    private void StartMessageMotion()
    {
        if (sendTransition is not null)
            foreach (var message in arrivingMessages)
                if (Transcript.ContainerFromItem(message) is ListViewItem reply)
                    ElementCompositionPreview.GetElementVisual(reply).Opacity = 0;
        if (sendTransition is { Started: false, Destination: { } row } transition
            && Transcript.ContainerFromItem(row) is ListViewItem container
            && container.ContentTemplateRoot is FrameworkElement root
            && root.FindName("MessageSurface") is UIElement surface
            && root.FindName("MessageText") is UIElement text
            && root.FindName("MessageTime") is UIElement time)
        {
            transition.Started = true;
            transition.Timestamp = time;
            ElementCompositionPreview.GetElementVisual(time).Opacity = 0;
            var surfaceStarted = transition.Surface.TryStart(surface);
            var textStarted = transition.Content.TryStart(text);
            if (!surfaceStarted || !textStarted)
                CancelSendTransition();
        }
        if (sendTransition is null)
            RevealReplies(motionSettings.AnimationsEnabled);
    }

    private void RevealReplies(bool animate)
    {
        foreach (var message in arrivingMessages)
        {
            if (Transcript.ContainerFromItem(message) is ListViewItem item)
            {
                var visual = ElementCompositionPreview.GetElementVisual(item);
                visual.Opacity = 1;
                if (animate)
                    visual.StartAnimation("Opacity", Fade(visual.Compositor, 0, 1, FeedbackMotion));
            }
            AnnounceMessage(message);
        }
        arrivingMessages.Clear();
    }

    private void SendTransitionCompleted(ConnectedAnimation sender, object args)
    {
        if (sendTransition is { } transition
            && (transition.Surface == sender || transition.Content == sender)
            && ++transition.CompletedParts == 2)
        {
            sendExpiry?.Stop();
            sendTransition = null;
            transition.Surface.Completed -= SendTransitionCompleted;
            transition.Content.Completed -= SendTransitionCompleted;
            SendCaptureLayer.Children.Remove(transition.Snapshot);
            SendCaptureLayer.Children.Remove(transition.SourceSurface);
            RevealTimestamp(transition, motionSettings.AnimationsEnabled);
            RevealReplies(motionSettings.AnimationsEnabled);
            UpdateAffordances();
        }
    }

    private void TranscriptContainerChanging(ListViewBase sender, ContainerContentChangingEventArgs args)
    {
        var visual = ElementCompositionPreview.GetElementVisual(args.ItemContainer);
        if (!args.InRecycleQueue)
        {
            if (sendTransition is not null && args.Item is MessageRow incoming
                && arrivingMessages.Contains(incoming))
                visual.Opacity = 0;
            return;
        }
        visual.StopAnimation("Opacity");
        visual.Opacity = 1;
        if (args.Item is MessageRow row && row == sendTransition?.Destination)
            CancelSendTransition();
    }

    private void MotionSizeChanged(object sender, SizeChangedEventArgs args) => CancelSendTransition();

    private static void RevealTimestamp(SendTransition transition, bool animate)
    {
        if (transition.Timestamp is not { } timestamp)
            return;
        var visual = ElementCompositionPreview.GetElementVisual(timestamp);
        visual.StopAnimation("Opacity");
        visual.Opacity = 1;
        if (animate)
            visual.StartAnimation("Opacity", Fade(visual.Compositor, 0, 1, FeedbackMotion));
        transition.Timestamp = null;
    }

    private void CancelSendTransition(bool revealWithMotion = false)
    {
        sendExpiry?.Stop();
        RevealReplies(revealWithMotion);
        if (sendTransition is not { } transition)
            return;
        sendTransition = null;
        transition.Surface.Completed -= SendTransitionCompleted;
        transition.Content.Completed -= SendTransitionCompleted;
        transition.Surface.Cancel();
        transition.Content.Cancel();
        RevealTimestamp(transition, false);
        SendCaptureLayer.Children.Remove(transition.Snapshot);
        SendCaptureLayer.Children.Remove(transition.SourceSurface);
        UpdateAffordances();
    }

    private void MotionSettingsChanged(UISettings sender, UISettingsAnimationsEnabledChangedEventArgs args)
    {
        DispatcherQueue.TryEnqueue(() =>
        {
            if (disposed)
                return;
            CancelSendTransition();
            arrivingMessages.Clear();
            ConfigureVisibilityMotion();
            if (!motionSettings.AnimationsEnabled)
            {
                var pane = ElementCompositionPreview.GetElementVisual(HistoryPane);
                pane.StopAnimation("Opacity");
                pane.Opacity = 1;
                if (Transcript.ItemsPanelRoot is Panel panel)
                    foreach (var child in panel.Children)
                    {
                        var visual = ElementCompositionPreview.GetElementVisual(child);
                        visual.StopAnimation("Opacity");
                        visual.Opacity = 1;
                    }
            }
        });
    }

    private void DisposeMotion()
    {
        arrivingMessages.Clear();
        CancelSendTransition();
        if (OperatingSystem.IsWindowsVersionAtLeast(10, 0, 19041))
            motionSettings.AnimationsEnabledChanged -= MotionSettingsChanged;
        Loaded -= MotionLoaded;
        SizeChanged -= MotionSizeChanged;
        Shell.PaneOpening -= PaneOpening;
        Shell.PaneClosing -= PaneClosing;
        Transcript.ContainerContentChanging -= TranscriptContainerChanging;
        foreach (var element in new UIElement[] { LatestSurface, EmptyState })
        {
            ElementCompositionPreview.SetImplicitShowAnimation(element, null);
            ElementCompositionPreview.SetImplicitHideAnimation(element, null);
        }
    }
}
