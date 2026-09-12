using System.ComponentModel;
using System.Linq;
using Arut.Bindings;
using Arut.Ffi;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Windows.System;

namespace Arut.Surface.Windows;

public sealed partial class ChatView : UserControl
{
    private bool observingModel;

    public ChatModel Model { get; }

    public ChatView(ChatModel model)
    {
        Model = model;
        InitializeComponent();
    }

    private async void Send(object sender, RoutedEventArgs args)
    {
        await Model.Send();
    }

    private async void ComposerKeyDown(object sender, KeyRoutedEventArgs args)
    {
        if (args.Key != VirtualKey.Enter) return;
        args.Handled = true;
        await Model.Send();
    }

    private void NewChat(object sender, RoutedEventArgs args)
    {
        StartNewChat();
    }

    private void StartNewChat()
    {
        Model.NewChat();
        History.SelectedItem = null;
        Composer.Focus(FocusState.Programmatic);
    }

    private void SelectChat(object sender, SelectionChangedEventArgs args)
    {
        if (History.SelectedItem is not ChatSummary chat) return;
        if (chat.Id != Model.SelectedChatId) Model.SelectChat(chat.Id);
        if (Navigation.DisplayMode != NavigationViewDisplayMode.Expanded) Navigation.IsPaneOpen = false;
        Composer.Focus(FocusState.Programmatic);
    }

    private void NewChatAccelerator(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        StartNewChat();
        args.Handled = true;
    }

    private void TogglePaneAccelerator(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        Navigation.IsPaneOpen = !Navigation.IsPaneOpen;
        args.Handled = true;
    }

    private void EscapeAccelerator(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (Navigation.IsPaneOpen && Navigation.DisplayMode != NavigationViewDisplayMode.Expanded)
        {
            Navigation.IsPaneOpen = false;
        }
        else
        {
            Composer.Focus(FocusState.Keyboard);
        }

        args.Handled = true;
    }

    private void ViewLoaded(object sender, RoutedEventArgs args)
    {
        if (!observingModel)
        {
            Model.PropertyChanged += ModelPropertyChanged;
            observingModel = true;
        }

        UpdateVisualState(scrollToLatest: true);
        Composer.Focus(FocusState.Programmatic);
    }

    private void ViewUnloaded(object sender, RoutedEventArgs args)
    {
        if (!observingModel) return;
        Model.PropertyChanged -= ModelPropertyChanged;
        observingModel = false;
    }

    private void ModelPropertyChanged(object? sender, PropertyChangedEventArgs args)
    {
        DispatcherQueue.TryEnqueue(() =>
            UpdateVisualState(scrollToLatest: args.PropertyName == nameof(ChatModel.State)));
    }

    private void UpdateVisualState(bool scrollToLatest)
    {
        var state = Model.State;
        var isSending = state.Status == ChatStatus.Sending;
        var hasMessages = state.Messages.Length > 0;
        var selected = Model.History.FirstOrDefault(chat => chat.Id == Model.SelectedChatId);

        ConversationTitle.Text = selected?.Title ?? "New conversation";
        EmptyState.Visibility = hasMessages ? Visibility.Collapsed : Visibility.Visible;
        SendingStatus.Visibility = isSending ? Visibility.Visible : Visibility.Collapsed;
        ErrorPresenter.Visibility = string.IsNullOrWhiteSpace(state.Error) ? Visibility.Collapsed : Visibility.Visible;
        ErrorText.Text = state.Error;
        Composer.IsEnabled = !isSending;
        SendButton.IsEnabled = !isSending && !string.IsNullOrWhiteSpace(Model.Draft);
        SendLabel.Text = isSending ? "Sending" : "Send";

        if (scrollToLatest && hasMessages)
        {
            Messages.ScrollIntoView(state.Messages[^1]);
        }
    }
}
