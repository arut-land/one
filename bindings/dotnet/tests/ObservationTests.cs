using System;
using System.Collections.Generic;
using System.Threading;
using Arut.Bindings;
using Xunit;

public class ObservationTests
{
    [Fact]
    public void BurstReplacementAndDisposal()
    {
        var previous = SynchronizationContext.Current;
        var context = new QueueContext();
        SynchronizationContext.SetSynchronizationContext(context);
        try
        {
            int value = 0, reads = 0, cancellations = 0;
            Action<ulong> notify = _ => { };
            using var observer = new ObservableState<int>(() => { reads++; return value; }, callback =>
            {
                notify = callback;
                value = 1;
                return new Cleanup(() => cancellations++);
            });
            Assert.Equal(1, observer.Value);
            var initialReads = reads;
            for (ulong i = 1; i <= 10000; i++) { value++; notify(i); }
            Assert.Single(context.Queue);
            context.Drain();
            Assert.Equal(10001, observer.Value);
            Assert.Equal(initialReads + 1, reads);
            var stale = notify;
            observer.Observe(() => value, callback =>
            {
                value = 42;
                notify = callback;
                return new Cleanup(() => cancellations++);
            });
            Assert.Equal(42, observer.Value);
            Assert.Equal(1, cancellations);
            stale(10001);
            Assert.Empty(context.Queue);
            notify(1);
            observer.Dispose();
            observer.Dispose();
            context.Drain();
            Assert.Equal(2, cancellations);
            Assert.Equal(42, observer.Value);
        }
        finally { SynchronizationContext.SetSynchronizationContext(previous); }
    }

    private sealed class QueueContext : SynchronizationContext
    {
        public readonly Queue<Action> Queue = new();
        public override void Post(SendOrPostCallback callback, object? state) => Queue.Enqueue(() => callback(state));
        public void Drain() { while (Queue.TryDequeue(out var action)) action(); }
    }
    private sealed class Cleanup(Action action) : IDisposable
    {
        public void Dispose() => action();
    }
}
