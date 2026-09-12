# 0008: Generated service layer

## Status

Accepted and partially implemented.

## Context

Protobuf service declarations previously generated message types only. Every feature then repeated connection traits, procedure paths, request encoding, response decoding, transport selection, and server dispatch. That boilerplate allowed the declared service and its actual HTTP implementation to drift.

Product code also needs the same typed service interface for direct in-process execution and remote execution. Sending local calls through byte framing would add work and contradict the zero-serialization path established by ADR 0007.

## Decision

Generate Arut's transport-neutral Rust service layer from Protobuf descriptors with a project-owned `prost_build::ServiceGenerator`.

For every service, generation produces:

- an object-safe typed service trait;
- a typed client with direct and remote targets;
- direct calls that pass generated messages without encoding;
- remote calls over an object-safe `RpcChannel`;
- an erased service router for isolation boundaries;
- canonical procedure names;
- service and method descriptors, including streaming shape.

The RPC runtime defines requests, responses, metadata, typed statuses, boxed futures and streams, channel shapes, registry dispatch, and descriptors. It has no feature, product, HTTP, Tokio, native SDK, or BoltFFI dependency.

Network protocols such as Connect or gRPC and raw transports such as HTTP, IPC, QUIC, or relay are adapters around `RpcChannel`. Product features never construct those adapters or contain endpoint paths.

Generated service descriptors are the static input to capability-manifest construction. Runtime registration adds implementation availability, limits, permissions, and route support. Schema presence alone never claims that a service is available.

Feature code continues to implement service meaning, validation, authority behavior, typed product outcomes, reducers, and conflict policy. Generation does not infer persistence, retries, optimistic transitions, queueability, or native lifecycle behavior.

Protobuf files are discovered recursively and deterministically. The generated package include tree replaces handwritten Rust module files and explicit build-script registration.

## Consequences

- Adding a Protobuf package requires no Rust module or build-script edit.
- Client and server procedure names cannot drift.
- Local composition remains typed and avoids serialization.
- Remote dispatch serializes exactly at the isolation boundary.
- All four Protobuf RPC streaming shapes have one generated API model.
- Expected product failures remain typed responses; `RpcStatus` represents invocation failures.
- The project owns compatibility and conformance tests for generated APIs.
- Connect, gRPC, browser, and native transport adapters can evolve without changing feature services.
- Runtime and session composition remains explicit rather than using linker registration or a global service locator.
