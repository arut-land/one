using Arut.Ffi;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
namespace Arut.Surface.Windows;
public sealed partial class ChatView : UserControl
{
    private readonly ProductSessionHandle session;
    private ChatHandle chat;
    private ComposerHandle composer;
    private System.IDisposable? chatSubscription;
    private System.IDisposable? composerSubscription;
    private readonly System.IDisposable listSubscription;
    public ChatView(ProductSessionHandle session)
    {
        this.session = session;
        chat = session.Chat();
        composer = chat.Composer();
        InitializeComponent();
        var list = session.Conversations();
        listSubscription = list.ListChanges(_ => DispatcherQueue.TryEnqueue(() => History.ItemsSource = list.State()));
        Bind(chat);
        Unloaded += (_, _) => { chatSubscription?.Dispose(); composerSubscription?.Dispose(); listSubscription.Dispose(); };
    }
    private void Bind(ChatHandle next)
    {
        chatSubscription?.Dispose(); composerSubscription?.Dispose();
        chat = next; composer = chat.Composer();
        chatSubscription = chat.ChatChanges(_ => DispatcherQueue.TryEnqueue(() => Transcript.ItemsSource = chat.State().Messages));
        composerSubscription = composer.ComposerChanges(_ => DispatcherQueue.TryEnqueue(() => { if (Composer.Text != composer.State().Text) Composer.Text = composer.State().Text; }));
    }
    private async void Send(object sender, RoutedEventArgs args) => await chat.Send(composer.State().Text);
    private async void ReplaceDraft(object sender, TextChangedEventArgs args) { if (Composer.Text != composer.State().Text) await composer.Replace(Composer.Text); }
    private void NewChat(object sender, RoutedEventArgs args) => Bind(session.NewChat());
    private void SelectChat(object sender, SelectionChangedEventArgs args) { if (History.SelectedItem is ChatSummary summary && session.SelectChat(summary.Id) is ChatHandle next) Bind(next); }
}
