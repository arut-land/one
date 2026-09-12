using Arut.Bindings;
using Arut.Surface.Windows;
using Microsoft.UI.Xaml;

namespace Arut.Windows;

public sealed partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();
        var model = new ChatModel();
        Content = new ChatView(model);
        Closed += (_, _) => model.Dispose();
    }

}
