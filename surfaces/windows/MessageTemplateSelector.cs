using Arut.Ffi;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Arut.Surface.Windows;

public sealed class MessageTemplateSelector : DataTemplateSelector
{
    public DataTemplate? AssistantTemplate { get; set; }
    public DataTemplate? UserTemplate { get; set; }

    protected override DataTemplate? SelectTemplateCore(object item) =>
        item is ChatMessage { Role: ChatRole.User } ? UserTemplate : AssistantTemplate;
}
