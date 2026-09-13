using Arut.Surface.Windows;
using Microsoft.UI.Xaml;

namespace Arut.Windows;

public sealed class MainWindow : Window
{
    public MainWindow()
    {
        Title = L10n.AppName();
        AppWindow.SetIcon(System.IO.Path.Combine(AppContext.BaseDirectory, "Assets", "Arut.ico"));
        AppWindow.TitleBar.PreferredTheme = Microsoft.UI.Windowing.TitleBarTheme.UseDefaultAppMode;
        SystemBackdrop = new Microsoft.UI.Xaml.Media.MicaBackdrop();
        AppWindow.Resize(new global::Windows.Graphics.SizeInt32(1100, 760));
        if (AppWindow.Presenter is Microsoft.UI.Windowing.OverlappedPresenter presenter)
        {
            presenter.PreferredMinimumWidth = 420;
            presenter.PreferredMinimumHeight = 480;
        }
        var session = Arut_ffi.CreateProductSession(Guid.NewGuid().ToString());
        var view = new ChatView(session);
        Content = view;
        ExtendsContentIntoTitleBar = true;
        AppWindow.TitleBar.PreferredHeightOption = Microsoft
            .UI
            .Windowing
            .TitleBarHeightOption
            .Standard;
        SetTitleBar(view.WindowTitleBar);
        AppWindow.TitleBar.ButtonBackgroundColor = Microsoft.UI.Colors.Transparent;
        AppWindow.TitleBar.ButtonInactiveBackgroundColor = Microsoft.UI.Colors.Transparent;
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
}
