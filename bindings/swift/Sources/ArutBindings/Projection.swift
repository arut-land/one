import Foundation
import Observation

// What every generated handle already offers: a projection to read, a stream of
// revisions, a typed error named by its Fluent id, and intents. That contract is
// the same for every feature, so the loop over it is written once, here, and a
// surface holds layout, intents and platform lifecycle only (ADR 0007, ADR 0021).

/// A projection read now, and re-read on every revision its handle reports.
///
/// Observation tracks the properties a `body` actually reads, so a view binds to
/// `value` and re-renders only when what it reads changed.
@Observable
public final class Projection<Value> {
    public private(set) var value: Value

    @ObservationIgnored private let read: () -> Value

    public init(_ read: @escaping () -> Value) {
        self.read = read
        self.value = read()
    }

    /// Re-reads the projection. Cheap: the read is a copy out of Rust's cell.
    public func refresh() {
        value = read()
    }

    /// Follows a generated revision stream until the owning task is cancelled.
    ///
    /// The first read happens before the first revision, so a projection that
    /// changed between `init` and the subscription is not missed.
    nonisolated(nonsending) public func follow(_ changes: AsyncStream<UInt64>) async {
        refresh()
        for await _ in changes {
            refresh()
        }
    }
}

/// Append-only keyed rows, read by cursor, with one rewind rule.
///
/// Rows arrive through `after(lastId)`, so a revision costs the rows it added
/// rather than the whole list. A source that no longer holds the row this cursor
/// stands on has rebound -- an editor bridge points one handle at another
/// subject -- and there is nothing to append to, so the cursor starts over.
@Observable
public final class Rows<Row> {
    public private(set) var rows: [Row] = []

    @ObservationIgnored private let identify: (Row) -> UInt64
    @ObservationIgnored private let after: (UInt64) -> [Row]

    public init(id: @escaping (Row) -> UInt64, after: @escaping (UInt64) -> [Row]) {
        self.identify = id
        self.after = after
        self.rows = after(0)
    }

    public var isEmpty: Bool { rows.isEmpty }

    public func refresh() {
        guard let last = rows.last else {
            rows = after(0)
            return
        }
        let cursor = identify(last)
        guard cursor > 0 else {
            rows = after(0)
            return
        }
        // One row of overlap is the rewind test: the source still holds the row
        // this cursor stands on, or it is showing something else entirely.
        let tail = after(cursor - 1)
        guard let first = tail.first, identify(first) == cursor else {
            rows = after(0)
            return
        }
        if tail.count > 1 {
            rows += tail.dropFirst()
        }
    }

    /// Follows a generated revision stream until the owning task is cancelled.
    nonisolated(nonsending) public func follow(_ changes: AsyncStream<UInt64>) async {
        refresh()
        for await _ in changes {
            refresh()
        }
    }
}

/// Follows one revision stream, applying everything that revision touches.
///
/// A scope publishes one revision for its whole projection, so a screen that
/// reads several things off the same handle -- a state, a row cursor, the typed
/// error -- takes one subscription and applies them together, rather than one
/// subscription each. The apply runs once before the first revision, so a change
/// between construction and subscription is not missed.
nonisolated(nonsending) public func observing(_ changes: AsyncStream<UInt64>, apply: () -> Void) async {
    apply()
    for await _ in changes {
        apply()
    }
}

/// Runs a handle's initialize-then-follow lifetime until the task is cancelled.
///
/// Cancellation is the only way out: SwiftUI cancels the task that owns the
/// view, which ends the awaits and releases the subscription behind them.
nonisolated(nonsending) public func following(
    initialize: () async throws -> Void,
    follow: () async throws -> Void,
    onFailure: ((any Error) -> Void)? = nil
) async {
    do {
        try await initialize()
        try await follow()
    } catch is CancellationError {
        return
    } catch {
        onFailure?(error)
    }
}

/// Suppresses the write-back an edit of our own caused.
///
/// A projection echoes what a surface just wrote, and applying that echo back to
/// the control the person is using would fight them. Anything applied inside
/// `applying` is ours, and the follower skips it.
public final class EchoGuard {
    private var depth = 0

    public init() {}

    public var isApplying: Bool { depth > 0 }

    @discardableResult
    public func applying<T>(_ body: () -> T) -> T {
        depth += 1
        defer { depth -= 1 }
        return body()
    }
}

/// A text field bound to a projection the core owns.
///
/// Writing `text` echoes locally and reaches Rust immediately, which coalesces
/// rapid edits behind one in-flight write and echoes the result back. The echo
/// is taken only while nothing local is unacknowledged, so the field never
/// reverts a keystroke the person has already typed.
@Observable
public final class Draft {
    public var text: String {
        get { stored }
        set {
            guard newValue != stored else { return }
            stored = newValue
            unacknowledged += 1
            Task { await self.write(newValue) }
        }
    }

    private var stored: String

    @ObservationIgnored private var unacknowledged = 0
    @ObservationIgnored private let replace: (String) async throws -> Void

    /// Called with whatever a write failed on. A `var` so an owner that only
    /// exists after its own stored properties can still wire it up.
    @ObservationIgnored public var onFailure: ((any Error) -> Void)?

    public init(
        _ text: String = "",
        replace: @escaping (String) async throws -> Void,
        onFailure: ((any Error) -> Void)? = nil
    ) {
        self.stored = text
        self.replace = replace
        self.onFailure = onFailure
    }

    public var isEmpty: Bool { stored.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }

    /// Takes what the core echoed back, unless a local write is still in flight.
    public func absorb(remote: String) {
        guard unacknowledged == 0, remote != stored else { return }
        stored = remote
    }

    private func write(_ text: String) async {
        defer { unacknowledged -= 1 }
        do {
            try await replace(text)
        } catch is CancellationError {
            return
        } catch {
            onFailure?(error)
        }
    }
}

/// A handle that names its current error by Fluent id (ADR 0016, ADR 0022).
///
/// Each argument arrives as the name that selects it and the value it carries,
/// in the order every generated catalog interpolates them, so a positional
/// formatter passes the values straight through and a formatter that wants the
/// names already has them.
///
/// `nonisolated` because the generated handles conform to it from another
/// module and carry their own isolation; the module's default main-actor
/// isolation must not reach across that conformance.
public nonisolated protocol ErrorSource {
    func errorKey() -> String?
    func errorArgs() -> [ErrorArg]
}

extension ErrorSource {
    /// What the core calls this error's arguments, in the order it emits them.
    public func errorArgNames() -> [String] { errorArgs().map(\.name) }

    /// The sentence for whatever error this source is holding, or `nil`.
    ///
    /// The core hands over a message id and its arguments, never a sentence, so
    /// resolution happens here -- in the same String Catalog and with the same
    /// negotiation as every other label. String Catalog formats are positional,
    /// and the core emits the arguments in the order the message declares them.
    public func localized(table: String = "Localizable", bundle: Bundle) -> String? {
        guard let key = errorKey() else { return nil }
        let format = String(localized: String.LocalizationValue(key), table: table, bundle: bundle)
        let arguments = errorArgs()
        guard !arguments.isEmpty else { return format }
        return String(format: format, arguments: arguments.map { $0.value as CVarArg })
    }
}

/// The last instant a date can name, in epoch milliseconds: 9999-12-31T23:59:59.999Z.
private let latestRepresentableMilliseconds: UInt64 = 253_402_300_799_999

/// An epoch-millisecond stamp as a date, or `nil` when there is no such instant.
///
/// Zero is "not stamped" and anything past the last representable date is a
/// value no clock produced; one bounds rule, so no surface writes the bound.
public func acceptedAt(_ epochMilliseconds: UInt64) -> Date? {
    guard epochMilliseconds > 0, epochMilliseconds <= latestRepresentableMilliseconds else { return nil }
    return Date(timeIntervalSince1970: Double(epochMilliseconds) / 1_000)
}
