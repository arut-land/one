import ArutBindings
import Testing

@MainActor
struct ObservationTests {
    @Test
    func burstReadsLatestSnapshotOnce() async throws {
        var value = 0
        var reads = 0
        var notify: ((UInt64) -> Void)?
        let observation = ObservableState(read: { reads += 1; return value }, subscribe: {
            notify = $0
            return {}
        })
        let initialReads = reads
        for revision in 1...10_000 {
            value = revision
            notify?(UInt64(revision))
        }
        try await waitUntil { observation.state == 10_000 }
        #expect(reads == initialReads + 1)
        observation.stopObserving()
    }

    @Test
    func subscriptionClosesInitialReadGap() {
        var value = 0
        let observation = ObservableState(read: { value }, subscribe: { _ in
            value = 42
            return {}
        })
        #expect(observation.state == 42)
    }

    @Test
    func replacingAndStoppingDiscardQueuedNotifications() async throws {
        var oldNotify: ((UInt64) -> Void)?
        var newNotify: ((UInt64) -> Void)?
        var cancellations = 0
        let observation = ObservableState(read: { 1 }, subscribe: {
            oldNotify = $0
            return { cancellations += 1 }
        })
        oldNotify?(1)
        var value = 2
        observation.observe(read: { value }, subscribe: {
            newNotify = $0
            return { cancellations += 1 }
        })
        #expect(cancellations == 1)
        oldNotify?(2)
        value = 3
        newNotify?(1)
        try await waitUntil { observation.state == 3 }
        observation.stopObserving()
        #expect(cancellations == 2)
        value = 4
        oldNotify?(3)
        newNotify?(2)
        for _ in 0..<10 { await Task.yield() }
        #expect(observation.state == 3)
        observation.stopObserving()
        #expect(cancellations == 2)
    }

    @Test
    func deinitializationCancelsSubscription() async {
        var cancellations = 0
        var observation: ObservableState<Int>? = ObservableState(read: { 1 }, subscribe: { _ in
            return { cancellations += 1 }
        })
        weak var retained = observation
        await Task.yield()
        observation = nil
        #expect(retained == nil)
        #expect(cancellations == 1)
    }

    private func waitUntil(_ predicate: () -> Bool) async throws {
        let clock = ContinuousClock()
        let deadline = clock.now.advanced(by: .seconds(5))
        while !predicate(), clock.now < deadline {
            try await Task.sleep(for: .milliseconds(1))
        }
        #expect(predicate())
    }
}
