using System.Collections.ObjectModel;
using System.Globalization;
using Arut.Ffi;
using CommunityToolkit.Mvvm.ComponentModel;

namespace Arut.Bindings;

// What every generated handle already offers: a projection to read, an
// IAsyncEnumerable of revisions, a typed error named by its Fluent id, and
// intents. That contract is the same for every feature, so the pump over it is
// written once, here, and a view model holds intents and screen state only
// (ADR 0007, ADR 0021).

/// <summary>
/// A projection read now, and re-read on every revision its handle reports.
/// </summary>
public sealed partial class Projection<T> : ObservableObject, IAsyncDisposable
{
    private readonly Func<T> read;
    private readonly Func<CancellationToken, IAsyncEnumerable<ulong>> changes;
    private readonly CancellationTokenSource lifetime = new();
    private Task running = Task.CompletedTask;
    private bool disposed;

    public Projection(Func<T> read, Func<CancellationToken, IAsyncEnumerable<ulong>> changes)
    {
        this.read = read;
        this.changes = changes;
        Value = read();
    }

    [ObservableProperty]
    public partial T Value { get; set; }

    /// <summary>Starts the pump on the caller's context, which in this app is
    /// the UI thread, so every projection read lands there too.</summary>
    public void Start() => running = FollowAsync();

    /// <summary>Re-reads the projection now.</summary>
    public void Refresh()
    {
        if (!disposed)
            Value = read();
    }

    private async Task FollowAsync()
    {
        try
        {
            await foreach (var _ in changes(lifetime.Token))
                Refresh();
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested) { }
    }

    public async ValueTask DisposeAsync()
    {
        if (disposed)
            return;
        disposed = true;
        lifetime.Cancel();
        try
        {
            await running;
        }
        catch (OperationCanceledException) { }
        lifetime.Dispose();
    }
}

/// <summary>
/// Append-only keyed rows, read by cursor, with one rewind rule.
/// </summary>
/// <remarks>
/// Rows arrive through <c>after(lastId)</c>, so a revision costs the rows it
/// added rather than the whole list, and existing items keep their identity --
/// no clear-and-repopulate churn to reset selection or scroll. A source that no
/// longer holds the row this cursor stands on has rebound, and there is nothing
/// to append to, so the cursor starts over.
/// </remarks>
public sealed class Rows<T> : ObservableCollection<T>
{
    private readonly Func<T, ulong> id;
    private readonly Func<ulong, IReadOnlyList<T>> after;

    public Rows(Func<T, ulong> id, Func<ulong, IReadOnlyList<T>> after)
    {
        this.id = id;
        this.after = after;
        Refresh();
    }

    /// <summary>Reads whatever the source added, and returns whether it added any.</summary>
    public bool Refresh()
    {
        var mark = Count == 0 ? 0UL : id(this[^1]);
        if (mark == 0)
            return Reset(after(0));
        // One row of overlap is the rewind test: the source still holds the row
        // this cursor stands on, or it is showing something else entirely.
        var tail = after(mark - 1);
        if (tail.Count == 0 || id(tail[0]) != mark)
            return Reset(after(0));
        for (var index = 1; index < tail.Count; index++)
            Add(tail[index]);
        return tail.Count > 1;
    }

    private bool Reset(IReadOnlyList<T> rows)
    {
        if (Count == 0 && rows.Count == 0)
            return false;
        Clear();
        foreach (var row in rows)
            Add(row);
        return true;
    }
}

/// <summary>
/// Brings a bound collection to what a projection now publishes, in place.
/// </summary>
/// <remarks>
/// A list that is cleared and refilled rebuilds every container, drops the
/// selection through -1 and resets the pane's scroll offset. Rust publishes an
/// ordered list with a key per row, so the rows that stayed keep their
/// containers and only what actually moved, arrived or left is reported.
/// </remarks>
public static class Reconcile
{
    public static void Apply<T, TKey>(
        ObservableCollection<T> target,
        IReadOnlyList<T> source,
        Func<T, TKey> key
    )
        where TKey : notnull
    {
        var wanted = new HashSet<TKey>(source.Count);
        foreach (var row in source)
            wanted.Add(key(row));
        for (var index = target.Count - 1; index >= 0; index--)
            if (!wanted.Contains(key(target[index])))
                target.RemoveAt(index);
        for (var index = 0; index < source.Count; index++)
        {
            var row = source[index];
            var at = IndexOf(target, key(row), key, index);
            if (at < 0)
            {
                target.Insert(index, row);
                continue;
            }
            if (at != index)
                target.Move(at, index);
            if (!EqualityComparer<T>.Default.Equals(target[index], row))
                target[index] = row;
        }
    }

    private static int IndexOf<T, TKey>(
        ObservableCollection<T> target,
        TKey key,
        Func<T, TKey> keyOf,
        int from
    )
        where TKey : notnull
    {
        for (var index = from; index < target.Count; index++)
            if (key.Equals(keyOf(target[index])))
                return index;
        return -1;
    }
}

/// <summary>Runs a handle's initialize-then-follow lifetime, cancellably.</summary>
public static class Following
{
    public static async Task RunAsync(
        Func<CancellationToken, Task> initialize,
        Func<CancellationToken, Task> follow,
        CancellationToken cancellationToken
    )
    {
        try
        {
            await initialize(cancellationToken);
            await follow(cancellationToken);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested) { }
    }
}

/// <summary>
/// Suppresses the write-back an edit of our own caused -- a draft the core
/// echoes, a selection the list reports back -- so applying it never fights the
/// person using the control.
/// </summary>
public sealed class EchoGuard
{
    private int depth;

    public bool IsApplying => depth > 0;

    /// <summary>Everything applied until the scope is disposed is ours.</summary>
    public EchoScope Applying() => new(this);

    internal void Enter() => depth++;

    internal void Leave() => depth--;
}

/// <summary>One <see cref="EchoGuard"/> application, ended by disposal.</summary>
public readonly struct EchoScope : IDisposable
{
    private readonly EchoGuard? guard;

    internal EchoScope(EchoGuard guard)
    {
        this.guard = guard;
        guard.Enter();
    }

    public void Dispose() => guard?.Leave();
}

/// <summary>
/// A text box bound to a projection the core owns.
/// </summary>
/// <remarks>
/// Writing <see cref="Text"/> echoes locally and then writes; Rust coalesces
/// rapid edits behind one in-flight request and echoes the result back.
/// <see cref="Absorb"/> takes that echo only while nothing local is
/// unacknowledged, so the box never reverts a keystroke already typed.
/// </remarks>
public sealed partial class Draft : ObservableObject
{
    private readonly Func<string> read;
    private readonly Func<string, Task> replace;
    private readonly EchoGuard echo = new();
    private int unacknowledged;

    public Draft(Func<string> read, Func<string, Task> replace)
    {
        this.read = read;
        this.replace = replace;
        using (echo.Applying())
            Text = read();
    }

    [ObservableProperty]
    public partial string Text { get; set; }

    /// <summary>Called with whatever a write failed on, if anything wants it.</summary>
    public Action<Exception>? Faulted { get; init; }

    public void Absorb(string remote)
    {
        if (unacknowledged > 0 || remote == Text)
            return;
        using (echo.Applying())
            Text = remote;
    }

    partial void OnTextChanged(string value)
    {
        if (echo.IsApplying)
            return;
        _ = WriteAsync(value);
    }

    private async Task WriteAsync(string value)
    {
        unacknowledged++;
        var acknowledged = false;
        try
        {
            await replace(value);
            acknowledged = true;
        }
        catch (OperationCanceledException) { }
        catch (Exception exception)
        {
            Faulted?.Invoke(exception);
        }
        finally
        {
            unacknowledged--;
            // Revisions can arrive before the write continuation, including a
            // send clearing the draft. Reread once all local edits are settled
            // so ignoring an in-flight echo cannot lose the last projection.
            if (acknowledged && unacknowledged == 0)
                Absorb(read());
        }
    }
}

/// <summary>
/// A handle that names its current error by Fluent id (ADR 0016, ADR 0022).
/// </summary>
/// <remarks>
/// The generated handles are sealed and implement nothing of ours, so the
/// key-and-arguments overload of <see cref="ErrorText"/>.<c>Describe</c> is what
/// a view model calls with them; this interface is for anything of our own that
/// carries an error.
/// </remarks>
public interface IErrorSource
{
    string? ErrorKey();

    ErrorArg[] ErrorArgs();

    /// <summary>What the core calls those arguments, in the same order.</summary>
    string[] ErrorArgNames() => [.. ErrorArgs().Select(argument => argument.Name)];
}

/// <summary>The sentence for whatever error a source is holding.</summary>
public static class ErrorText
{
    /// <param name="get">Resolves one resource name to its format string.</param>
    public static string Describe(IErrorSource source, Func<string, string> get) =>
        Describe(source.ErrorKey(), source.ErrorArgs(), get);

    /// <remarks>
    /// The core hands over a message id and its arguments, never a sentence, so
    /// resolution happens here. Fluent ids are hyphenated; resw names are the
    /// same ids with underscores. Each argument carries the name that selects it
    /// beside its value; the formats are positional, and the core emits the
    /// arguments in the order the message declares them.
    /// </remarks>
    public static string Describe(
        string? key,
        IReadOnlyList<ErrorArg> arguments,
        Func<string, string> get
    )
    {
        if (key is null)
            return "";
        var format = get(key.Replace('-', '_'));
        if (arguments.Count == 0)
            return format;
        var values = new object[arguments.Count];
        for (var index = 0; index < arguments.Count; index++)
            values[index] = arguments[index].Value;
        return string.Format(CultureInfo.CurrentCulture, format, values);
    }
}

/// <summary>Instants the core sends as epoch milliseconds.</summary>
public static class Time
{
    /// <summary>The last instant a date can name: 9999-12-31T23:59:59.999Z.</summary>
    private const ulong LatestRepresentableMilliseconds = 253_402_300_799_999UL;

    /// <summary>
    /// A stamp as a local date, or <c>null</c> when there is no such instant:
    /// zero is "not stamped", and past the last representable date is a value no
    /// clock produced. One bounds rule, so no surface writes the bound.
    /// </summary>
    public static DateTimeOffset? AcceptedAt(ulong epochMilliseconds) =>
        epochMilliseconds is 0 or > LatestRepresentableMilliseconds
            ? null
            : DateTimeOffset.FromUnixTimeMilliseconds((long)epochMilliseconds).ToLocalTime();
}
