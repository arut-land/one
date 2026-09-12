# 0001: Initial bootstrap vertical slice

## Status

Superseded by the Chat feature and in part by [ADR 0008](0008-generated-service-layer.md). This records the original bootstrap architecture; later ADRs define the current generated service and distributed session model.

## Decision

The first feature crosses the same boundaries intended for later product work:

1. The feature domain implements the portable response rule.
2. The feature product layer owns the client state machine.
3. `protocols` generates Rust request/response types from the Protobuf source contract.
4. `bindings/ffi` exposes the product session through BoltFFI.
5. Ecosystem adapters publish generated state through native observable primitives without redefining it.
6. `surfaces` contain the native experience and its deployable composition root without owning product behavior.

Surfaces and runtimes remain independent. Generated FFI artifacts live in ignored `bindings/generated`, are never edited manually, and may only be imported by ecosystem bindings.

The original slice used a feature connection and in-memory transport. ADR 0008 replaced both with generated direct service clients, which pass generated Rust values without serialization. Generated remote clients encode the same values at the `RpcChannel` boundary.

Surface implementation and packaging stay together while each surface produces one artifact. A surface may introduce internal `shared` and target directories when it produces multiple artifacts.
