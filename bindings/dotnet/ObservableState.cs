using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Threading;

namespace Arut.Bindings;

public sealed class ObservableState<T> : IDisposable, INotifyPropertyChanged
{
    private readonly SynchronizationContext? context = SynchronizationContext.Current;
    private readonly Func<Action, bool>? dispatch;
    private readonly IEqualityComparer<T> comparer;
    private readonly object gate = new();
    private Func<T> read;
    private IDisposable? subscription;
    private long generation;

    // Count invalidations locally: revisions can restart on a replacement source.
    private long requestedRevision;
    private bool disposed;
    private int refreshPending;
    private T value;

    public ObservableState(
        Func<T> read,
        Func<Action<ulong>, IDisposable> subscribe,
        Func<Action, bool>? dispatch = null,
        IEqualityComparer<T>? comparer = null
    )
    {
        this.dispatch = dispatch;
        this.comparer = comparer ?? EqualityComparer<T>.Default;
        this.read = read;
        value = read();
        subscription = subscribe(_ => Invalidate(0));
        Set(0, Volatile.Read(ref requestedRevision), read());
    }

    public T Value
    {
        get
        {
            lock (gate)
                return value;
        }
    }

    public event Action? Changed;
    public event PropertyChangedEventHandler? PropertyChanged;

    public void RefreshNow()
    {
        Func<T> reader;
        long observedGeneration;
        lock (gate)
        {
            if (disposed)
                return;
            reader = read;
            observedGeneration = generation;
        }
        Set(observedGeneration, Volatile.Read(ref requestedRevision), reader());
    }

    public void Observe(Func<T> read, Func<Action<ulong>, IDisposable> subscribe)
    {
        long observedGeneration;
        IDisposable? previous;
        lock (gate)
        {
            if (disposed)
                throw new ObjectDisposedException(nameof(ObservableState<T>));
            observedGeneration = ++generation;
            previous = subscription;
            subscription = null;
            this.read = read;
        }
        previous?.Dispose();
        var next = subscribe(_ => Invalidate(observedGeneration));
        bool retain;
        lock (gate)
        {
            retain = !disposed && observedGeneration == generation;
            if (retain)
                subscription = next;
        }
        if (retain)
            Set(observedGeneration, Volatile.Read(ref requestedRevision), read());
        else
            next.Dispose();
    }

    public void StopObserving()
    {
        IDisposable? previous;
        lock (gate)
        {
            generation++;
            previous = subscription;
            subscription = null;
        }
        previous?.Dispose();
    }

    private void Invalidate(long observedGeneration)
    {
        lock (gate)
        {
            if (disposed || observedGeneration != generation)
                return;
            Interlocked.Increment(ref requestedRevision);
        }
        ScheduleRefresh();
    }

    private void ScheduleRefresh()
    {
        if (Interlocked.CompareExchange(ref refreshPending, 1, 0) != 0)
            return;
        if (dispatch is not null)
        {
            if (!dispatch(Refresh))
                Volatile.Write(ref refreshPending, 0);
        }
        else if (context is not null)
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
        Func<T> reader;
        long observedGeneration;
        lock (gate)
        {
            if (disposed || subscription is null)
            {
                // Stopping observation also discards work queued before unsubscribe.
                // Release the guard so a later Observe can schedule fresh work.
                Volatile.Write(ref refreshPending, 0);
                return;
            }
            reader = read;
            observedGeneration = generation;
        }
        var revision = Volatile.Read(ref requestedRevision);
        try
        {
            Set(observedGeneration, revision, reader());
        }
        finally
        {
            Volatile.Write(ref refreshPending, 0);
            if (revision != Volatile.Read(ref requestedRevision))
                ScheduleRefresh();
        }
    }

    private void Set(long observedGeneration, long revision, T next)
    {
        Action? changed;
        lock (gate)
        {
            // A read may run on the thread pool without a UI context. Do not
            // publish it if replacement or disposal happened during that read.
            if (disposed || observedGeneration != generation || revision != requestedRevision)
                return;
            if (comparer.Equals(value, next))
                return;
            value = next;
            changed = Changed;
        }
        changed?.Invoke();
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(Value)));
    }

    public void Dispose()
    {
        IDisposable? previous;
        lock (gate)
        {
            if (disposed)
                return;
            disposed = true;
            previous = subscription;
            subscription = null;
        }
        previous?.Dispose();
    }
}
