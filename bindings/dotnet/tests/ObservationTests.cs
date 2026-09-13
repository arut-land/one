using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading;
using Arut.Bindings;
using Xunit;

public class ObservationTests
{
    [Fact]
    public void StructuralComparerKeepsEqualDecodedCollectionsAndNotifiesRealChanges()
    {
        var values = new[] { 1, 2 };
        using var observer = new ObservableState<int[]>(
            () => values.ToArray(),
            _ => new Cleanup(() => { }),
            comparer: new ArrayComparer()
        );
        var initial = observer.Value;
        int changes = 0;
        observer.Changed += () => changes++;

        observer.RefreshNow();
        Assert.Same(initial, observer.Value);
        Assert.Equal(0, changes);

        values = new[] { 2, 1 };
        observer.RefreshNow();
        Assert.Equal(values, observer.Value);
        Assert.Equal(1, changes);
        observer.RefreshNow();
        Assert.Equal(1, changes);
    }

    private sealed class ArrayComparer : IEqualityComparer<int[]>
    {
        public bool Equals(int[]? left, int[]? right) =>
            ReferenceEquals(left, right)
            || (left is not null && right is not null && left.SequenceEqual(right));

        public int GetHashCode(int[] value)
        {
            var hash = new HashCode();
            foreach (var item in value)
                hash.Add(item);
            return hash.ToHashCode();
        }
    }

    [Fact]
    public void StoppingObservationDiscardsQueuedReadsAndCanResume()
    {
        var queue = new Queue<Action>();
        Action<ulong> notify = _ => { };
        int value = 1,
            reads = 0;
        int Read()
        {
            reads++;
            return value;
        }
        IDisposable Subscribe(Action<ulong> callback)
        {
            notify = callback;
            return new Cleanup(() => { });
        }
        using var observer = new ObservableState<int>(
            Read,
            Subscribe,
            action =>
            {
                queue.Enqueue(action);
                return true;
            }
        );
        value = 2;
        notify(1);
        observer.StopObserving();
        var stoppedReads = reads;
        queue.Dequeue()();
        Assert.Equal(stoppedReads, reads);
        Assert.Equal(1, observer.Value);
        observer.Observe(Read, Subscribe);
        Assert.Equal(2, observer.Value);
        value = 3;
        notify(2);
        queue.Dequeue()();
        Assert.Equal(3, observer.Value);
    }

    [Fact]
    public void HiddenSourceStopsCallbacksAndResumesWithLatestValue()
    {
        var queue = new Queue<Action>();
        Action<ulong> notify = _ => { };
        int value = 1,
            cancellations = 0;
        IDisposable Subscribe(Action<ulong> callback)
        {
            notify = callback;
            return new Cleanup(() => cancellations++);
        }
        using var observer = new ObservableState<int>(
            () => value,
            Subscribe,
            action =>
            {
                queue.Enqueue(action);
                return true;
            }
        );
        observer.StopObserving();
        value = 2;
        notify(1);
        Assert.Empty(queue);
        Assert.Equal(1, cancellations);
        observer.Observe(() => value, Subscribe);
        Assert.Equal(2, observer.Value);
        value = 3;
        notify(2);
        queue.Dequeue()();
        Assert.Equal(3, observer.Value);
    }

    [Fact]
    public void ExplicitDispatcherCoalescesAndNotifiesBindings()
    {
        var queue = new Queue<Action>();
        Action<ulong> notify = _ => { };
        int value = 1,
            changes = 0;
        bool accept = false;
        using var observer = new ObservableState<int>(
            () => value,
            callback =>
            {
                notify = callback;
                return new Cleanup(() => { });
            },
            action =>
            {
                if (!accept)
                    return false;
                queue.Enqueue(action);
                return true;
            }
        );
        observer.PropertyChanged += (_, args) =>
        {
            Assert.Equal("Value", args.PropertyName);
            changes++;
        };
        value = 2;
        notify(1); // A closing dispatcher can refuse delivery without throwing.
        Assert.Empty(queue);
        accept = true;
        for (ulong i = 2; i < 10002; i++)
            notify(i);
        Assert.Single(queue);
        observer.RefreshNow();
        Assert.Equal(2, observer.Value);
        notify(10002);
        Assert.Single(queue); // An explicit read must not clear the queued-work guard.
        queue.Dequeue()();
        Assert.Equal(1, changes);
        value = 3;
        notify(10003);
        observer.Dispose();
        queue.Dequeue()();
        Assert.Equal(1, changes);
    }

    [Fact]
    public void BurstReplacementAndDisposal()
    {
        var previous = SynchronizationContext.Current;
        var context = new QueueContext();
        SynchronizationContext.SetSynchronizationContext(context);
        try
        {
            int value = 0,
                reads = 0,
                cancellations = 0;
            Action<ulong> notify = _ => { };
            using var observer = new ObservableState<int>(
                () =>
                {
                    reads++;
                    return value;
                },
                callback =>
                {
                    notify = callback;
                    value = 1;
                    return new Cleanup(() => cancellations++);
                }
            );
            Assert.Equal(1, observer.Value);
            var initialReads = reads;
            for (ulong i = 1; i <= 10000; i++)
            {
                value++;
                notify(i);
            }
            Assert.Single(context.Queue);
            context.Drain();
            Assert.Equal(10001, observer.Value);
            Assert.Equal(initialReads + 1, reads);
            var stale = notify;
            observer.Observe(
                () => value,
                callback =>
                {
                    value = 42;
                    notify = callback;
                    return new Cleanup(() => cancellations++);
                }
            );
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
        finally
        {
            SynchronizationContext.SetSynchronizationContext(previous);
        }
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
            using var observer = new ObservableState<int>(
                () =>
                {
                    var action = duringRead;
                    duringRead = null;
                    action?.Invoke();
                    return value;
                },
                callback =>
                {
                    notify = callback;
                    return new Cleanup(() => { });
                }
            );
            value = 99;
            duringRead = dispose
                ? observer.Dispose
                : () => observer.Observe(() => 42, _ => new Cleanup(() => { }));
            notify(1);
            context.Drain();
            Assert.Equal(dispose ? 7 : 42, observer.Value);
        }
        finally
        {
            SynchronizationContext.SetSynchronizationContext(previous);
        }
    }

    [Fact]
    public void ReplacementRevisionRestartDoesNotLosePendingChange()
    {
        var previous = SynchronizationContext.Current;
        var context = new QueueContext();
        SynchronizationContext.SetSynchronizationContext(context);
        try
        {
            Action<ulong> oldNotify = _ => { },
                newNotify = _ => { };
            Action? duringRead = null;
            using var observer = new ObservableState<int>(
                () =>
                {
                    var action = duringRead;
                    duringRead = null;
                    action?.Invoke();
                    return 7;
                },
                callback =>
                {
                    oldNotify = callback;
                    return new Cleanup(() => { });
                }
            );
            int value = 42;
            duringRead = () =>
            {
                observer.Observe(
                    () => value,
                    callback =>
                    {
                        newNotify = callback;
                        return new Cleanup(() => { });
                    }
                );
                value = 43;
                newNotify(1); // Same revision as the previous source.
            };
            oldNotify(1);
            context.Drain();
            Assert.Equal(43, observer.Value);
        }
        finally
        {
            SynchronizationContext.SetSynchronizationContext(previous);
        }
    }

    [Fact]
    public void SetupReadCannotOverwriteNewerRefresh()
    {
        var previous = SynchronizationContext.Current;
        var context = new QueueContext();
        SynchronizationContext.SetSynchronizationContext(context);
        try
        {
            int value = 0,
                reads = 0;
            Action<ulong> notify = _ => { };
            using var observer = new ObservableState<int>(
                () =>
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
                },
                callback =>
                {
                    notify = callback;
                    return new Cleanup(() => { });
                }
            );
            Assert.Equal(1, observer.Value);
        }
        finally
        {
            SynchronizationContext.SetSynchronizationContext(previous);
        }
    }

    private sealed class QueueContext : SynchronizationContext
    {
        public readonly Queue<Action> Queue = new();

        public override void Post(SendOrPostCallback callback, object? state) =>
            Queue.Enqueue(() => callback(state));

        public void Drain()
        {
            while (Queue.TryDequeue(out var action))
                action();
        }
    }

    private sealed class Cleanup(Action action) : IDisposable
    {
        public void Dispose() => action();
    }
}
