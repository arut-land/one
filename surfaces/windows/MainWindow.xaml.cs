using System.Runtime.InteropServices;
using Arut.Surface.Windows;
using Microsoft.UI;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;

namespace Arut.Windows;

public sealed class MainWindow : Window
{
    private XamlRoot? root;

    [DllImport("user32.dll", ExactSpelling = true)]
    private static extern uint GetDpiForWindow(nint window);

    public MainWindow()
    {
        Title = L10n.Get(L10n.AppName);
        AppWindow.SetIcon(System.IO.Path.Combine(AppContext.BaseDirectory, "Assets", "Arut.ico"));
        AppWindow.TitleBar.PreferredTheme = TitleBarTheme.UseDefaultAppMode;
        SystemBackdrop = new Microsoft.UI.Xaml.Media.MicaBackdrop();
        var scale = GetDpiForWindow(Win32Interop.GetWindowFromWindowId(AppWindow.Id)) / 96.0;
        var workArea = DisplayArea
            .GetFromWindowId(AppWindow.Id, DisplayAreaFallback.Nearest)
            .WorkArea;
        // Window APIs use physical pixels; the layout's sizes are DIPs.
        AppWindow.Resize(
            new global::Windows.Graphics.SizeInt32(
                Math.Min((int)Math.Round(1100 * scale), workArea.Width),
                Math.Min((int)Math.Round(760 * scale), workArea.Height)
            )
        );
        SetMinimumSize(scale);
        var session = Arut_ffi.CreateProductSession(Guid.NewGuid().ToString());
        var view = new ChatView(session);
        Content = view;
        view.Loaded += (_, _) =>
        {
            root = view.XamlRoot;
            root.Changed += RootChanged;
            SetMinimumSize(root.RasterizationScale);
        };
        view.Unloaded += (_, _) =>
        {
            if (root is not null)
                root.Changed -= RootChanged;
            root = null;
        };
        ExtendsContentIntoTitleBar = true;
        AppWindow.TitleBar.PreferredHeightOption = TitleBarHeightOption.Standard;
        SetTitleBar(view.WindowTitleBar);
        AppWindow.TitleBar.ButtonBackgroundColor = Colors.Transparent;
        AppWindow.TitleBar.ButtonInactiveBackgroundColor = Colors.Transparent;
        var closing = false;
        var cleanedUp = false;
        AppWindow.Closing += async (_, args) =>
        {
            if (cleanedUp)
                return;
            args.Cancel = true;
            if (closing)
                return;
            closing = true;
            await view.DisposeAsync();
            session.Dispose();
            cleanedUp = true;
            Close();
        };
    }

    private void RootChanged(XamlRoot root, XamlRootChangedEventArgs args) =>
        SetMinimumSize(root.RasterizationScale);

    private void SetMinimumSize(double scale)
    {
        if (AppWindow.Presenter is not OverlappedPresenter presenter)
            return;
        var width = (int)Math.Round(420 * scale);
        var height = (int)Math.Round(480 * scale);
        if (presenter.PreferredMinimumWidth != width)
            presenter.PreferredMinimumWidth = width;
        if (presenter.PreferredMinimumHeight != height)
            presenter.PreferredMinimumHeight = height;
    }
}
