import ArutBindings

// The generated handles already publish a Fluent message id and the arguments
// it takes, under exactly the names `ErrorSource` declares (ADR 0016, ADR 0022).
// One retroactive conformance each is the whole adapter: nothing is wrapped,
// nothing is forwarded, and a new scope handle costs one more line.
//
// `@retroactive` because the type is the FFI package's and the protocol is the
// binding package's; this module is where the two meet.

extension ChatHandle: @retroactive ErrorSource {}

extension ComposerHandle: @retroactive ErrorSource {}
