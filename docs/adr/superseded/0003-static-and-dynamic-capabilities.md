# 0003: Static and dynamic capabilities

## Status

Accepted and partially implemented. Generated service descriptors, runtime registration metadata, capability discovery, and typed Chat Composer availability are implemented; compatibility ranges, permissions enforcement, and capability update streaming remain pending.

## Context

Rust can prove that locally composed code implements required services, but it cannot know whether an independently updated remote node is online, authorized, compatible, or currently has a tool installed. A boolean capability registry would spread conditional behavior through features and surfaces.

## Decision

Use two related capability mechanisms.

Static capabilities are narrow feature-owned Rust traits. Core features depend on the smallest useful capability bundle, and runtimes compose concrete implementations. There is no global runtime god trait or service locator.

Dynamic capabilities are reported by a typed manifest generated from Protobuf service definitions and implementation metadata. The manifest contains service versions, limits, permissions, extensions, and current availability. It is negotiated at session connection time and may be updated during the session.

Product code converts negotiated support into feature-specific availability projections. Bindings expose those projections through native types. Native views do not inspect raw capability strings or duplicate compatibility rules.

Queueability, expiry, idempotency, revision checks, and optimistic behavior are typed command policies. Their wire representation remains explicit so remote peers can enforce them.

## Consequences

- Invalid local runtime compositions fail at compile time.
- Remote compatibility remains dynamic and version-aware.
- Adding a runtime implementation does not add branches to product or surface code.
- Generated manifests avoid a second handwritten capability registry.
- Some runtime failure remains unavoidable because availability can change after negotiation.
