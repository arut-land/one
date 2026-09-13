import Combine

@MainActor
public final class ObservableState<Value>: ObservableObject {
    @Published public private(set) var state: Value
    // Deinitialization may run off the main actor. This closure only cancels
    // the thread-safe stream, consumer task, and generated FFI subscription.
    nonisolated(unsafe) private var cancel: (() -> Void)?

    public init(_ state: Value) {
        self.state = state
    }

    public init(
        read: @escaping () -> Value,
        subscribe: (@escaping (UInt64) -> Void) -> (() -> Void)
    ) {
        self.state = read()
        observe(read: read, subscribe: subscribe)
    }

    public func observe(
        read: @escaping () -> Value,
        subscribe: (@escaping (UInt64) -> Void) -> (() -> Void)
    ) {
        stopObserving()
        // Notifications invalidate a snapshot; only the newest pending signal
        // matters. Buffer before hopping to the main actor, not one task per signal.
        let (stream, continuation) = AsyncStream<UInt64>.makeStream(bufferingPolicy: .bufferingNewest(1))
        let unsubscribe = subscribe { continuation.yield($0) }
        // Close the read/subscribe gap even if subscription emits no initial event.
        state = read()
        let task = Task { @MainActor [weak self] in
            for await _ in stream {
                guard !Task.isCancelled, let self else { break }
                self.state = read()
            }
        }
        cancel = {
            task.cancel()
            continuation.finish()
            unsubscribe()
        }
    }

    public func receive(_ value: Value) {
        state = value
    }

    public func stopObserving() {
        cancel?()
        cancel = nil
    }

    deinit { cancel?() }
}
