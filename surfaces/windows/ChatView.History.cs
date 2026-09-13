using Microsoft.UI.Xaml;

namespace Arut.Surface.Windows;

public sealed partial class ChatView
{
    private bool updatingHistory;
    private bool historyRefreshPending;

    private void QueueHistoryRefresh()
    {
        if (disposed || historyRefreshPending)
            return;
        historyRefreshPending = DispatcherQueue.TryEnqueue(() =>
        {
            historyRefreshPending = false;
            RefreshHistory();
        });
    }

    private void RefreshHistory()
    {
        if (disposed)
            return;
        updatingHistory = true;
        try
        {
            var rows = history
                .Value.Where(row =>
                    row.Title.Contains(Search.Text, StringComparison.CurrentCultureIgnoreCase)
                )
                .ToArray();
            var existingRows = Summaries.ToDictionary(row => row.Id);
            for (var i = 0; i < rows.Length; i++)
            {
                if (!existingRows.TryGetValue(rows[i].Id, out var existing))
                    Summaries.Insert(i, new(rows[i]));
                else
                {
                    if (Summaries[i] != existing)
                        Summaries.Move(Summaries.IndexOf(existing), i);
                    existing.Update(rows[i]);
                }
            }
            while (Summaries.Count > rows.Length)
                Summaries.RemoveAt(Summaries.Count - 1);
            History.SelectedItem = Summaries.FirstOrDefault(row => row.Id == current.Id);
            HistoryEmpty.Text =
                Search.Text.Length == 0 ? HistoryEmptyLabel : L10n.ConversationSearchEmpty();
            HistoryEmpty.Visibility =
                Summaries.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
            ConversationTitle.Text =
                history.Value.FirstOrDefault(row => row.Id == current.Id).Title
                ?? NewConversationLabel;
        }
        finally
        {
            updatingHistory = false;
        }
    }
}
