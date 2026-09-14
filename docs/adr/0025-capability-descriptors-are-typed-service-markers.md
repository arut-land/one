---
status: accepted
amends: 0008
---

# Capability descriptors are typed service markers, not a runtime table

The capability manifest was built from a runtime registry: each service registered a `ServiceMetadata` value, an `RwLock` table held the registrations, and the product answered "is the composer available" by comparing package and service name strings written by hand. The strings were the defect, because nothing failed when one drifted from the proto. We decided that the generator emits one marker type per service carrying its `ServiceDescriptor` as an associated constant, that tuples of markers implement `ServiceSet`, and that `manifest::<S>()` and `CapabilityManifest::availability::<S>()` are the only ways to ask. `Node::serve` takes the set as a type parameter. No service or package name is written by hand anywhere in the product.

## Considered options

- **Keep the runtime metadata table.** Rejected: three types and a lock holding values the type system already knows at compile time, and it left the name strings in the product.
- **Generate the availability types now.** Deferred: `prost-reflect` can walk the descriptor set and emit a typed availability struct per service, which is ADR 0008's stated intent, but the approval feature is the second consumer that justifies the generator. The markers are the half that is needed today, and they are what that generator would emit.

## Consequences

- `ServiceRuntimeMetadata`, `ServiceMetadata`, `ServiceRegistration` and the registrations behind them are deleted; `RpcRegistry` is the route map alone.
- `ServiceDescriptor::version` becomes `Version { major, minor }` and `compatible()` states ADR 0015's two-minor window, but nothing produces a minor other than zero yet, so the comparison is inert until a second minor exists.
- The proto fields (`available`, `unavailable_reason`, `permissions`, `limits`, `extensions`) are untouched, so a node may still report a service it hosts as unavailable.
