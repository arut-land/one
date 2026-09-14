using System.Collections.Specialized;
using System.ComponentModel;
using global::Windows.ApplicationModel.DataTransfer;
using global::Windows.System;
using global::Windows.UI.Core;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

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
        InitializeMotion();
        ViewModel.PropertyChanging += ConversationChanging;
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

    private void ConversationChanging(object? sender, PropertyChangingEventArgs args)
    {
        // Cancel while the old templates still exist. PropertyChanged is too
        // late: compiled bindings may already have recycled animation targets.
        if (args.PropertyName == nameof(ViewModel.Conversation))
            CancelSendTransition();
    }

    private void ConversationChanged(object? sender, PropertyChangedEventArgs args)
    {
        if (disposed || args.PropertyName != nameof(ViewModel.Conversation))
            return;
        arrivingMessages.Clear();
        CancelSendTransition();
        FollowMessages();
        followLatest = true;
        animateNextScroll = false;
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
        {
            ObserveMessageMotion(row);
        }
    }

    private void NewChat(XamlUICommand sender, ExecuteRequestedEventArgs args)
    {
        ViewModel.NewConversation();
        FocusComposer();
    }

    // Selection updates the view model; only activation transfers focus. Arrow
    // navigation and incoming projection updates leave focus in the history.
    private void ChatActivated(object sender, ItemClickEventArgs args)
    {
        if (disposed)
            return;
        FocusComposer();
    }

    private async void RenameConversation(object sender, RoutedEventArgs args)
    {
        if (disposed || sender is not MenuFlyoutItem { CommandParameter: ConversationRow row })
            return;

        var title = new TextBox
        {
            Header = Labels.RenameConversationTitle,
            Text = row.Title,
            SelectionStart = 0,
            SelectionLength = row.Title.Length,
        };
        AutomationProperties.SetName(title, Labels.RenameConversationTitle);
        var dialog = new ContentDialog
        {
            XamlRoot = XamlRoot,
            Title = Labels.RenameConversationTitle,
            Content = title,
            PrimaryButtonText = Labels.Save,
            CloseButtonText = Labels.Cancel,
            DefaultButton = ContentDialogButton.Primary,
            IsPrimaryButtonEnabled = !string.IsNullOrWhiteSpace(row.Title),
        };
        title.TextChanged += (_, _) =>
            dialog.IsPrimaryButtonEnabled = !string.IsNullOrWhiteSpace(title.Text);

        if (await dialog.ShowAsync() == ContentDialogResult.Primary)
            await ViewModel.RenameAsync(row.Id, title.Text);
    }

    private async void DeleteConversation(object sender, RoutedEventArgs args)
    {
        if (disposed || sender is not MenuFlyoutItem { CommandParameter: ConversationRow row })
            return;

        var dialog = new ContentDialog
        {
            XamlRoot = XamlRoot,
            Title = Labels.DeleteConversationTitle,
            Content = $"{row.Title}\n\n{Labels.DeleteConversationMessage}",
            PrimaryButtonText = Labels.DeleteConversation,
            CloseButtonText = Labels.Cancel,
            DefaultButton = ContentDialogButton.Close,
        };
        if (await dialog.ShowAsync() == ContentDialogResult.Primary)
            await ViewModel.DeleteAsync(row.Id);
    }

    private void HistoryKeyDown(object sender, KeyRoutedEventArgs args)
    {
        if (
            args.Key != VirtualKey.Application
            && (args.Key != VirtualKey.F10 || !IsDown(VirtualKey.Shift))
        )
            return;

        var focused = FocusManager.GetFocusedElement(XamlRoot) as DependencyObject;
        while (focused is not null && focused != History)
        {
            if (focused is ListViewItem item)
            {
                var root = item.ContentTemplateRoot as FrameworkElement;
                if (root?.ContextFlyout is { } flyout)
                {
                    flyout.ShowAt(root);
                    args.Handled = true;
                }
                return;
            }
            focused = VisualTreeHelper.GetParent(focused);
        }
    }

    private void AnnounceMessage(MessageRow row) =>
        FrameworkElementAutomationPeer.CreatePeerForElement(Transcript)?.RaiseNotificationEvent(
            AutomationNotificationKind.Other,
            AutomationNotificationProcessing.CurrentThenMostRecent,
            $"{row.Author}: {row.Text}",
            "IncomingMessage"
        );

    private void CopyMessage(XamlUICommand sender, ExecuteRequestedEventArgs args)
    {
        if (args.Parameter is not string text)
            return;
        Copy(text);
    }

    private void CopyFocusedMessage(
        KeyboardAccelerator sender,
        KeyboardAcceleratorInvokedEventArgs args
    )
    {
        var focused = FocusManager.GetFocusedElement(XamlRoot) as DependencyObject;
        // Let native text selection copy the selected range before offering
        // whole-message copy to a keyboard-focused container.
        if (focused is TextBlock { SelectedText.Length: > 0 })
            return;
        while (focused is not null && focused != Transcript)
        {
            if (focused is ListViewItem { Content: MessageRow message })
            {
                Copy(message.Text);
                args.Handled = true;
                return;
            }
            focused = VisualTreeHelper.GetParent(focused);
        }
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
        var command = ViewModel.Conversation.SendCommand;
        if (command.CanExecute(null))
        {
            PrepareSendTransition();
            command.Execute(null);
        }
    }

    // Button.Click runs before its bound command, capturing the current editor
    // before the accepted send clears it. Keyboard sends take the same path.
    private void SendClicked(object sender, RoutedEventArgs args) => PrepareSendTransition();

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
        DisposeMotion();
        ViewModel.PropertyChanging -= ConversationChanging;
        ViewModel.PropertyChanged -= ConversationChanged;
        messages?.CollectionChanged -= MessagesChanged;
        await ViewModel.DisposeAsync();
    }
}
