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

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void ReplacementOrDisposalDuringReadRejectsStaleSnapshot(bool dispose)
    {
        var previous = SynchronizationContext.Current;
        var context = new QueueContext();
        SynchronizationContext.SetSynchronizationContext(context);
        try
        {
            Action<ulong> notify = _ => { };
            Action? duringRead = null;
            int value = 7;
            using var observer = new ObservableState<int>(() =>
            {
                var action = duringRead;
                duringRead = null;
                action?.Invoke();
                return value;
            }, callback => { notify = callback; return new Cleanup(() => { }); });
            value = 99;
            duringRead = dispose ? observer.Dispose : () => observer.Observe(
                () => 42, _ => new Cleanup(() => { }));
            notify(1);
            context.Drain();
            Assert.Equal(dispose ? 7 : 42, observer.Value);
        }
        finally { SynchronizationContext.SetSynchronizationContext(previous); }
    }

    [Fact]
    public void ReplacementRevisionRestartDoesNotLosePendingChange()
    {
        var previous = SynchronizationContext.Current;
        var context = new QueueContext();
        SynchronizationContext.SetSynchronizationContext(context);
        try
        {
            Action<ulong> oldNotify = _ => { }, newNotify = _ => { };
            Action? duringRead = null;
            using var observer = new ObservableState<int>(() =>
            {
                var action = duringRead;
                duringRead = null;
                action?.Invoke();
                return 7;
            }, callback => { oldNotify = callback; return new Cleanup(() => { }); });
            int value = 42;
            duringRead = () =>
            {
                observer.Observe(() => value, callback =>
                {
                    newNotify = callback;
                    return new Cleanup(() => { });
                });
                value = 43;
                newNotify(1); // Same revision as the previous source.
            };
            oldNotify(1);
            context.Drain();
            Assert.Equal(43, observer.Value);
        }
        finally { SynchronizationContext.SetSynchronizationContext(previous); }
    }

    [Fact]
    public void SetupReadCannotOverwriteNewerRefresh()
    {
        var previous = SynchronizationContext.Current;
        var context = new QueueContext();
        SynchronizationContext.SetSynchronizationContext(context);
        try
        {
            int value = 0, reads = 0;
            Action<ulong> notify = _ => { };
            using var observer = new ObservableState<int>(() =>
            {
                var snapshot = value;
                if (++reads == 2)
                {
                    value = 1;
                    notify(1);
                    // Simulate a concurrent refresh finishing before the setup read.
                    context.Drain();
                }
                return snapshot;
            }, callback => { notify = callback; return new Cleanup(() => { }); });
            Assert.Equal(1, observer.Value);
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
