import Combine

@MainActor
public class ObservableState<Value>: ObservableObject {
    @Published public private(set) var state: Value
    // `deinit` is always nonisolated, even on a @MainActor class, so it
    // cannot touch actor-isolated storage under Swift 6 strict concurrency.
    // `cancel` only ever unsubscribes a closure and is never read concurrently
    // with the isolated methods below, so it is safe to exempt from isolation.
    nonisolated(unsafe) private var cancel: (() -> Void)?
    private var observing = false
    private var refreshPending = false
    private var generation: UInt64 = 0

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
        cancel?()
        generation &+= 1
        let generation = generation
        observing = true
        refreshPending = false
        cancel = subscribe { [weak self] _ in
            Task { @MainActor [weak self] in
                guard let self,
                      self.generation == generation,
                      !self.refreshPending else { return }
                self.refreshPending = true
                await Task.yield()
                if self.observing, self.generation == generation {
                    self.state = read()
                }
                self.refreshPending = false
            }
        }
    }

    public func receive(_ value: Value) {
        state = value
    }

    public func stopObserving() {
        observing = false
        generation &+= 1
        cancel?()
        cancel = nil
    }

    deinit {
        cancel?()
    }
}
