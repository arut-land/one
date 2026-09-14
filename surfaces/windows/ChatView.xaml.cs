using System.Collections.Specialized;
using System.ComponentModel;
using global::Windows.ApplicationModel.DataTransfer;
using global::Windows.System;
using global::Windows.UI.Core;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;

namespace Arut.Surface.Windows;

/// <summary>
/// The view. Everything it owns is a Windows concern: focus, keyboard, the
/// pane, the clipboard, and where the transcript is scrolled. State, commands
/// and every string come from the view model through compiled bindings.
/// </summary>
public sealed partial class ChatView : UserControl, IAsyncDisposable
{
    private INotifyCollectionChanged? messages;
    private bool disposed;
    private bool composing;

    public ChatView(ProductSessionHandle session)
    {
        ViewModel = new ChatViewModel(session);
        InitializeComponent();
        ViewModel.PropertyChanged += ConversationChanged;
        ViewModel.Start();
        FollowMessages();
        Composer.TextCompositionStarted += (_, _) => composing = true;
        Composer.TextCompositionEnded += (_, _) => composing = false;
        // `Keyboard` rather than `Programmatic`: the first thing a keyboard user
        // sees should be the focus rectangle, not an invisible caret.
        Loaded += (_, _) => Composer.Focus(FocusState.Keyboard);
    }

    public ChatViewModel ViewModel { get; }

    public TitleBar WindowTitleBar => AppTitleBar;

    private void ConversationChanged(object? sender, PropertyChangedEventArgs args)
    {
        if (disposed || args.PropertyName != nameof(ViewModel.Conversation))
            return;
        FollowMessages();
        followLatest = true;
        QueueScroll();
    }

    private void FollowMessages()
    {
        messages?.CollectionChanged -= MessagesChanged;
        messages = ViewModel.Conversation.Messages;
        messages.CollectionChanged += MessagesChanged;
    }

    private void MessagesChanged(object? sender, NotifyCollectionChangedEventArgs change)
    {
        if (disposed)
            return;
        QueueScroll();
        if (change.NewItems is null)
            return;
        foreach (MessageRow row in change.NewItems)
            if (!row.IsOutgoing)
                FrameworkElementAutomationPeer
                    .CreatePeerForElement(Transcript)
                    ?.RaiseNotificationEvent(
                        AutomationNotificationKind.Other,
                        AutomationNotificationProcessing.CurrentThenMostRecent,
                        $"{row.Author}: {row.Text}",
                        "IncomingMessage"
                    );
    }

    private void NewChat(XamlUICommand sender, ExecuteRequestedEventArgs args)
    {
        ViewModel.NewConversation();
        FocusComposer();
    }

    // The list's own selection is the only path into it; `SelectedIndex` is
    // two-way bound, so this only moves focus after a deliberate pick.
    private void ChatSelected(object sender, SelectionChangedEventArgs args)
    {
        if (disposed || History.FocusState == FocusState.Unfocused)
            return;
        FocusComposer();
    }

    private void CopyMessage(XamlUICommand sender, ExecuteRequestedEventArgs args)
    {
        if (args.Parameter is not string text)
            return;
        Copy(text);
    }

    private void CopySelectedMessage(
        KeyboardAccelerator sender,
        KeyboardAcceleratorInvokedEventArgs args
    )
    {
        if (Transcript.SelectedItem is not MessageRow message)
            return;
        Copy(message.Text);
        args.Handled = true;
    }

    private static void Copy(string text)
    {
        var data = new DataPackage();
        data.SetText(text);
        Clipboard.SetContent(data);
    }

    private void TitleBarToggle(TitleBar sender, object args) => ToggleHistory();

    private void ToggleHistory()
    {
        Shell.IsPaneOpen = !Shell.IsPaneOpen;
        if (Shell.IsPaneOpen)
            History.Focus(FocusState.Keyboard);
        else
            Composer.Focus(FocusState.Keyboard);
    }

    private void FocusComposer()
    {
        if (Shell.DisplayMode == SplitViewDisplayMode.Overlay)
            Shell.IsPaneOpen = false;
        Composer.Focus(FocusState.Keyboard);
    }

    private void FocusSearch(XamlUICommand sender, ExecuteRequestedEventArgs args)
    {
        Shell.IsPaneOpen = true;
        Search.Focus(FocusState.Keyboard);
    }

    private void ToggleHistoryShortcut(
        KeyboardAccelerator sender,
        KeyboardAcceleratorInvokedEventArgs args
    )
    {
        ToggleHistory();
        args.Handled = true;
    }

    private void FocusComposerShortcut(
        KeyboardAccelerator sender,
        KeyboardAcceleratorInvokedEventArgs args
    )
    {
        FocusComposer();
        args.Handled = true;
    }

    private void SendShortcut(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (composing)
            return;
        args.Handled = true;
        Send();
    }

    private void ComposerKeyDown(object sender, KeyRoutedEventArgs args)
    {
        if (
            args.Key != VirtualKey.Enter
            || composing
            || IsDown(VirtualKey.Shift)
            || IsDown(VirtualKey.Control)
            || IsDown(VirtualKey.Menu)
        )
            return;
        args.Handled = true;
        Send();
    }

    private void Send()
    {
        followLatest = true;
        Composer.Focus(FocusState.Keyboard);
        var command = ViewModel.Conversation.SendCommand;
        if (command.CanExecute(null))
            command.Execute(null);
    }

    private static bool IsDown(VirtualKey key) =>
        InputKeyboardSource.GetKeyStateForCurrentThread(key).HasFlag(CoreVirtualKeyStates.Down);

    private void ComposerFocusChanged(object sender, RoutedEventArgs args) =>
        VisualStateManager.GoToState(
            this,
            Composer.FocusState == FocusState.Unfocused ? "ComposerUnfocused" : "ComposerFocused",
            false
        );

    public async ValueTask DisposeAsync()
    {
        if (disposed)
            return;
        disposed = true;
        ViewModel.PropertyChanged -= ConversationChanged;
        messages?.CollectionChanged -= MessagesChanged;
        await ViewModel.DisposeAsync();
    }
}
