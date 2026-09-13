using System;
using System.Collections.Generic;
using System.Threading;
using System.Threading.Tasks;
using Arut.Surface.Windows;
using Xunit;

public sealed class ConversationCommandsTests
{
    [Fact]
    public void PendingEditsCoalesceWithoutCrossingTheSendBarrier()
    {
        var previous = SynchronizationContext.Current;
        var context = new QueueContext();
        SynchronizationContext.SetSynchronizationContext(context);
        try
        {
            var initialized = new TaskCompletionSource();
            var commands = new ConversationCommands(initialized.Task);
            var applied = new List<string>();
            Task Apply(string text)
            {
                applied.Add(text);
                return Task.CompletedTask;
            }

            for (var i = 0; i < 1000; i++)
            {
                var text = $"draft {i}";
                commands.Enqueue(() => Apply(text), edit: true);
            }
            var sent = commands.Enqueue(() => Apply("send draft 999"));
            commands.Enqueue(() => Apply("next draft"), edit: true);
            commands.Enqueue(() => Apply("next draft final"), edit: true);
            context.Drain();
            Assert.Empty(applied);

            initialized.SetResult();
            context.Drain();
            Assert.Equal(new[] { "draft 999", "send draft 999", "next draft final" }, applied);
            Assert.True(sent.IsCompletedSuccessfully);
            Assert.True(commands.Completion.IsCompletedSuccessfully);
        }
        finally
        {
            SynchronizationContext.SetSynchronizationContext(previous);
        }
    }

    [Fact]
    public void InFlightEditFinishesBeforeTheLatestQueuedEdit()
    {
        var previous = SynchronizationContext.Current;
        var context = new QueueContext();
        SynchronizationContext.SetSynchronizationContext(context);
        try
        {
            var commands = new ConversationCommands(Task.CompletedTask);
            var accepted = new TaskCompletionSource();
            var applied = new List<string>();
            commands.Enqueue(
                async () =>
                {
                    applied.Add("first started");
                    await accepted.Task;
                    applied.Add("first accepted");
                },
                edit: true
            );
            context.Drain();

            Task Apply(string text)
            {
                applied.Add(text);
                return Task.CompletedTask;
            }
            commands.Enqueue(() => Apply("intermediate"), edit: true);
            commands.Enqueue(() => Apply("latest"), edit: true);
            var closing = commands.Completion;
            context.Drain();
            Assert.False(closing.IsCompleted);
            Assert.Equal(new[] { "first started" }, applied);

            accepted.SetResult();
            context.Drain();
            Assert.Equal(new[] { "first started", "first accepted", "latest" }, applied);
            Assert.True(closing.IsCompletedSuccessfully);
        }
        finally
        {
            SynchronizationContext.SetSynchronizationContext(previous);
        }
    }

    private sealed class QueueContext : SynchronizationContext
    {
        private readonly Queue<Action> callbacks = new();

        public override void Post(SendOrPostCallback callback, object? state) =>
            callbacks.Enqueue(() => callback(state));

        public void Drain()
        {
            while (callbacks.TryDequeue(out var callback))
                callback();
        }
    }
}
