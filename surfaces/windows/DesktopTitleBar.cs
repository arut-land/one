using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Arut.Surface.Windows;

// WinUI copies AppWindow's physical caption insets into XAML GridLengths.
// Convert them to DIPs until the upstream TitleBar performs that conversion.
// Still open as of Windows App SDK 2.4.0; recheck TitleBar::UpdatePadding against
// https://github.com/microsoft/microsoft-ui-xaml/issues/10344 on the next upgrade.
public sealed class DesktopTitleBar : TitleBar
{
    private ColumnDefinition? left;
    private ColumnDefinition? right;
    private XamlRoot? root;
    private bool queued;

    public DesktopTitleBar()
    {
        Loaded += (_, _) =>
        {
            root = XamlRoot;
            root.Changed += RootChanged;
            UpdateInsets();
        };
        Unloaded += (_, _) =>
        {
            root?.Changed -= RootChanged;
            root = null;
        };
        SizeChanged += (_, _) => UpdateInsets();
    }

    protected override void OnApplyTemplate()
    {
        base.OnApplyTemplate();
        left = GetTemplateChild("LeftPaddingColumn") as ColumnDefinition;
        right = GetTemplateChild("RightPaddingColumn") as ColumnDefinition;
        UpdateInsets();
    }

    private void RootChanged(XamlRoot sender, XamlRootChangedEventArgs args) => UpdateInsets();

    private void UpdateInsets()
    {
        if (queued)
            return;
        queued = DispatcherQueue.TryEnqueue(() =>
        {
            queued = false;
            if (root is null || !root.IsHostVisible || left is null || right is null)
                return;
            var window = AppWindow.GetFromWindowId(root.ContentIslandEnvironment.AppWindowId);
            // Caption geometry is transient while minimizing/restoring. Keep
            // the last valid padding until the window supplies usable insets.
            if (window.Presenter is OverlappedPresenter { State: OverlappedPresenterState.Minimized })
                return;
            var caption = window.TitleBar;
            var leftInset = caption.LeftInset;
            var rightInset = caption.RightInset;
            // On restore the runtime can briefly report a negative inset
            // (observed RightInset = -19). GridLength requires nonnegative values.
            if (leftInset < 0 || rightInset < 0)
                return;
            var rtl = FlowDirection == FlowDirection.RightToLeft;
            left.Width = new GridLength(
                (rtl ? rightInset : leftInset) / root.RasterizationScale
            );
            right.Width = new GridLength(
                (rtl ? leftInset : rightInset) / root.RasterizationScale
            );
        });
    }
}
