# 0006: Product session and native bindings

## Status

Accepted and partially implemented. `ProductSession` owns the pending Chat Composer scope and established chat aggregates; native session adapters and shared route, lifecycle, capability, and replication context remain pending.

## Context

Every surface needs the same conversations, negotiated availability, routes, identity, and lifecycle. Independent feature bindings would create duplicate connections, subscriptions, manifests, and accidental session boundaries. Native platforms still require their own observation and presentation mechanisms.

## Decision

Each deployable composition root creates one scoped product session facade. The facade exposes typed feature projections and controllers while sharing connection, identity, capability manifest, route, lifecycle, and replication context.

Product owns projections, intents, reconciliation, and intent-specific optimistic behavior. It publishes bounded feature state instead of one global application snapshot.

Bindings adapt the session to native observation primitives and schedulers. They may own ecosystem presentation state and local write policy, but they do not select runtimes, duplicate product transitions, or expose raw generated FFI to surfaces.

Surfaces own widgets, navigation, focus, accessibility, permissions, and native lifecycle observation. Drafts remain local unless explicitly promoted to synchronized product state.

## Consequences

- Features share one coherent session and negotiated authority view.
- Command-only consumers can avoid unused reactive subscriptions.
- Native fidelity remains in platform code without duplicating product behavior.
- Session lifetime and disposal must be integrated with each platform lifecycle.
- The facade must remain typed and scoped rather than becoming a global service locator.
