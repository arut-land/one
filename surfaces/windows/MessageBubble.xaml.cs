using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Animation;

namespace Arut.Surface.Windows;

public sealed partial class MessageBubble : UserControl
{
    public static readonly DependencyProperty MessageProperty = DependencyProperty.Register(
        nameof(Message),
        typeof(MessageRow),
        typeof(MessageBubble),
        new PropertyMetadata(null, Changed)
    );
    public MessageRow? Message
    {
        get => (MessageRow?)GetValue(MessageProperty);
        set => SetValue(MessageProperty, value);
    }

    public MessageBubble()
    {
        InitializeComponent();
        Loaded += (_, _) => RevealReply();
    }

    internal FrameworkElement TransitionElement => Root;

    internal bool CanAnimateWithin(FrameworkElement viewport, double bottomInset)
    {
        var bounds = MessageText
            .TransformToVisual(viewport)
            .TransformBounds(new(0, 0, MessageText.ActualWidth, MessageText.ActualHeight));
        return bounds.Width >= 1
            && bounds.Height >= 1
            && bounds.Y >= 0
            && bounds.Bottom <= viewport.ActualHeight - bottomInset;
    }

    private static void Changed(DependencyObject sender, DependencyPropertyChangedEventArgs args) =>
        ((MessageBubble)sender).Refresh();

    private void Refresh()
    {
        if (Message is not { } message)
            return;
        MessageText.Text = message.Text;
        TimeText.Text = message.Time;
        TimeText.Visibility = message.Time.Length == 0 ? Visibility.Collapsed : Visibility.Visible;
        ToolTipService.SetToolTip(Bubble, message.FullTime);
        TimeGroup.Text = message.TimeGroup;
        TimeGroup.Visibility =
            message.TimeGroup.Length == 0 ? Visibility.Collapsed : Visibility.Visible;
        Bubble.Margin = message.IsOutgoing
            ? new Thickness(48, message.StartsGroup ? 10 : 2, 0, 0)
            : new Thickness(0, message.StartsGroup ? 10 : 2, 48, 0);
        VisualStateManager.GoToState(this, message.IsOutgoing ? "Outgoing" : "Incoming", false);
        RevealReply();
    }

    private void RevealReply()
    {
        if (!IsLoaded || Message is not { RevealOnLoad: true } message)
            return;
        message.RevealOnLoad = false;
        var fade = new FadeInThemeAnimation();
        Storyboard.SetTarget(fade, Bubble);
        var storyboard = new Storyboard();
        storyboard.Children.Add(fade);
        storyboard.Begin();
    }
}
