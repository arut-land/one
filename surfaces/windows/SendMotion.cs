using CommunityToolkit.WinUI;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;

namespace Arut.Surface.Windows;

// The helper matches layouts by ID. Only the background scales; text retains
// its glyph size. A frozen source lets the live editor clear and resize normally.
internal sealed class SendMotion : IDisposable
{
    private readonly Canvas layer;
    private readonly Grid source;
    private readonly CancellationTokenSource lifetime = new();
    private readonly DispatcherQueueTimer expiry;
    private readonly Action<SendMotion> finished;
    private TransitionHelper? transition;
    private bool disposed;

    public SendMotion(Canvas layer, Grid card, TextBox editor, Action<SendMotion> finished)
    {
        this.layer = layer;
        this.finished = finished;
        var origin = card.TransformToVisual(layer).TransformPoint(new());
        var textOrigin = editor.TransformToVisual(card).TransformPoint(new());
        source = new()
        {
            Width = card.ActualWidth,
            Height = card.ActualHeight,
            IsHitTestVisible = false,
        };
        var background = new Border
        {
            Background = card.Background,
            CornerRadius = card.CornerRadius,
        };
        var text = new TextBlock
        {
            Text = editor.Text.Trim(),
            FontFamily = editor.FontFamily,
            FontSize = editor.FontSize,
            Foreground = editor.Foreground,
            TextWrapping = TextWrapping.Wrap,
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Top,
            MaxWidth = Math.Max(1, editor.ActualWidth - editor.Padding.Left - editor.Padding.Right),
            MaxHeight = Math.Max(
                1,
                editor.ActualHeight - editor.Padding.Top - editor.Padding.Bottom
            ),
            Margin = new(
                textOrigin.X + editor.Padding.Left,
                textOrigin.Y + editor.Padding.Top,
                0,
                0
            ),
        };
        TransitionHelper.SetId(background, "background");
        TransitionHelper.SetId(text, "text");
        AutomationProperties.SetAccessibilityView(text, AccessibilityView.Raw);
        source.Children.Add(background);
        source.Children.Add(text);
        Canvas.SetLeft(source, origin.X);
        Canvas.SetTop(source, origin.Y);
        layer.Children.Add(source);
        layer.UpdateLayout();
        // A recycled or unrealized destination must never leave replies held.
        expiry = layer.DispatcherQueue.CreateTimer();
        expiry.Interval = TimeSpan.FromSeconds(1);
        expiry.IsRepeating = false;
        expiry.Tick += Expired;
        expiry.Start();
    }

    public MessageRow? Destination { get; set; }
    public bool HasStarted { get; private set; }
    public bool Completed { get; private set; }

    public async Task StartAsync(MessageBubble target)
    {
        if (disposed)
            return;
        HasStarted = true;
        transition = new()
        {
            Source = source,
            Target = target.TransitionElement,
            SourceToggleMethod = VisualStateToggleMethod.ByIsVisible,
            TargetToggleMethod = VisualStateToggleMethod.ByIsVisible,
            Duration = TimeSpan.FromMilliseconds(220),
            DefaultIndependentTranslation = new(0, 0),
            IndependentElementShowDelay = TimeSpan.FromMilliseconds(120),
            IndependentElementShowDuration = TimeSpan.FromMilliseconds(100),
            Configs =
            [
                new()
                {
                    Id = "background",
                    ScaleMode = ScaleMode.Scale,
                    OpacityTransitionProgressKey = new(.3, .3),
                },
                new()
                {
                    Id = "text",
                    ScaleMode = ScaleMode.None,
                    NormalizedCenterPoint = new(0, 0),
                    OpacityTransitionProgressKey = new(.5, .5),
                },
            ],
        };
        try
        {
            await transition.StartAsync(lifetime.Token, forceUpdateAnimatedElements: true);
            Completed = !lifetime.IsCancellationRequested;
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested) { }
        catch (Exception exception)
        {
            System.Diagnostics.Trace.TraceError(exception.ToString());
        }
        finally
        {
            Dispose();
        }
    }

    private void Expired(DispatcherQueueTimer sender, object args) => Dispose();

    public void Dispose()
    {
        if (disposed)
            return;
        disposed = true;
        expiry.Stop();
        expiry.Tick -= Expired;
        lifetime.Cancel();
        transition?.Reset(toInitialState: false);
        lifetime.Dispose();
        layer.Children.Remove(source);
        Destination = null;
        finished(this);
    }
}
