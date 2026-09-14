using System.Numerics;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Arut.Surface.Windows;

/// <summary>Content feedback driven by the native Button's visual states.
/// Button still owns pointer capture, keyboard activation, commands and focus.</summary>
public sealed class ActionButton : Button
{
    private VisualStateGroup? commonStates;
    private readonly Vector3Transition feedbackTransition = new() { Duration = TimeSpan.FromMilliseconds(83) };
    private bool motionEnabled = true;

    public bool MotionEnabled
    {
        get => motionEnabled;
        set
        {
            motionEnabled = value;
            UpdateMotion();
        }
    }

    protected override void OnApplyTemplate()
    {
        if (commonStates is not null)
            commonStates.CurrentStateChanged -= StateChanged;
        base.OnApplyTemplate();
        if (VisualTreeHelper.GetChildrenCount(this) > 0
            && VisualTreeHelper.GetChild(this, 0) is FrameworkElement root)
            commonStates = VisualStateManager.GetVisualStateGroups(root)
                .FirstOrDefault(group => group.Name == "CommonStates");
        if (commonStates is not null)
            commonStates.CurrentStateChanged += StateChanged;
        UpdateMotion();
    }

    private void StateChanged(object sender, VisualStateChangedEventArgs args) => UpdateMotion();

    private void UpdateMotion()
    {
        if (Content is not FrameworkElement content)
            return;
        content.CenterPoint = new((float)content.ActualWidth / 2, (float)content.ActualHeight / 2, 0);
        content.ScaleTransition = motionEnabled ? feedbackTransition : null;
        var scale = motionEnabled ? commonStates?.CurrentState?.Name switch
        {
            "Pressed" => 0.88f,
            "PointerOver" => 1.08f,
            _ => 1f,
        } : 1f;
        content.Scale = new Vector3(scale, scale, 1);
    }
}
