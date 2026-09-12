using System;
using System.Collections.Generic;
using System.Threading;

namespace Arut.Bindings;

public sealed class ObservableState<T> : IDisposable
{
    private readonly SynchronizationContext? context = SynchronizationContext.Current;
    private Func<T> read;
    private IDisposable subscription;
    private long generation;
    private long requestedRevision;
    private int disposed;
    private int refreshPending;
    private T value;

    public ObservableState(
        Func<T> read,
        Func<Action<ulong>, IDisposable> subscribe)
    {
        this.read = read;
        value = read();
        subscription = subscribe(Invalidate);
    }

    public T Value => value;

    public event Action? Changed;

    public void Observe(
        Func<T> read,
        Func<Action<ulong>, IDisposable> subscribe)
    {
        if (Volatile.Read(ref disposed) != 0) throw new ObjectDisposedException(nameof(ObservableState<T>));
        Interlocked.Increment(ref generation);
        subscription.Dispose();
        this.read = read;
        Set(read());
        var observedGeneration = Volatile.Read(ref generation);
        subscription = subscribe(revision => Invalidate(observedGeneration, revision));
    }

    private void Invalidate(ulong revision)
    {
        Invalidate(Volatile.Read(ref generation), revision);
    }

    private void Invalidate(long observedGeneration, ulong revision)
    {
        if (Volatile.Read(ref disposed) != 0 || observedGeneration != Volatile.Read(ref generation)) return;
        Interlocked.Exchange(ref requestedRevision, unchecked((long)revision));
        ScheduleRefresh();
    }

    private void ScheduleRefresh()
    {
        if (Interlocked.CompareExchange(ref refreshPending, 1, 0) != 0) return;
        if (context is not null)
        {
            context.Post(_ => Refresh(), null);
        }
        else
        {
            ThreadPool.QueueUserWorkItem(_ => Refresh());
        }
    }

    private void Refresh()
    {
        if (Volatile.Read(ref disposed) != 0) return;
        var revision = Volatile.Read(ref requestedRevision);
        Set(read());
        Volatile.Write(ref refreshPending, 0);
        if (revision != Volatile.Read(ref requestedRevision)) ScheduleRefresh();
    }

    private void Set(T next)
    {
        if (EqualityComparer<T>.Default.Equals(value, next)) return;
        value = next;
        Changed?.Invoke();
    }

    public void Dispose()
    {
        if (Interlocked.Exchange(ref disposed, 1) == 0) subscription.Dispose();
    }
}
