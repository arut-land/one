using Arut.Surface.Windows;
using Microsoft.UI.Xaml;

namespace Arut.Windows;

public sealed partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();
        var session = Arut_ffi.CreateProductSession("local-demo");
        Content = new ChatView(session);
        Closed += (_, _) => session.Dispose();
    }

}
