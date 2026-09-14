// Every generated FFI type reaches a surface that imports ArutBindings, with no
// rename layer of our own to keep in sync (ADR 0007).
@_exported import ArutFfi

// BoltFFI exports a class only when its Rust type is `Send + Sync`, checked at
// compile time, and its generated methods may be called from any thread. The
// Swift side does not yet say so, and a main-actor screen awaiting one of the
// generated async methods would otherwise be sending the handle across an
// isolation boundary. These conformances state BoltFFI's contract where every
// consumer sees it; they go away once the generator declares it.
extension ProductSessionHandle: @unchecked @retroactive Sendable {}
extension ConversationsHandle: @unchecked @retroactive Sendable {}
extension AvailabilityHandle: @unchecked @retroactive Sendable {}
extension ChatHandle: @unchecked @retroactive Sendable {}
extension ComposerHandle: @unchecked @retroactive Sendable {}
