using System;
using System.Threading.Tasks;

namespace Arut.Surface.Windows;

// Accessed on the UI thread. Only queued edits can be replaced; a send seals
// the preceding draft so subsequent typing remains after that send.
internal sealed class ConversationCommands(Task initialized)
{
    private sealed class Command(Func<Task> action)
    {
        public Func<Task> Action = action;
        public Task Completion = Task.CompletedTask;
    }

    private Command? pendingEdit;
    public Task Completion { get; private set; } = initialized;

    public Task Enqueue(Func<Task> action, bool edit = false)
    {
        if (edit && pendingEdit is { } pending)
        {
            pending.Action = action;
            return pending.Completion;
        }

        var command = new Command(action);
        pendingEdit = edit ? command : null;
        var previous = Completion;
        return Completion = command.Completion = Run();

        async Task Run()
        {
            // Publish the queue tail before a synchronous native completion.
            await Task.Yield();
            await previous;
            if (pendingEdit == command)
                pendingEdit = null;
            await command.Action();
        }
    }
}
