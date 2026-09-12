---
status: accepted
---

# Typed scopes and capability bundles instead of a service container

We want scoped services in the spirit of VS Code's instantiation service (node, workspace, conversation, operation) without its runtime lookup. We decided that each scope is a Rust struct constructed by a builder, holding `Arc`s to its parent and constructing its children, and that a feature's dependencies are a supertrait bundle of narrow ports with a blanket impl. The compiler proves a conversation cannot exist without a workspace and a feature cannot be built on a runtime missing one of its ports. Plug-in features register into scopes through generated code, never through a locator.

## Considered options

- **A container keyed by `TypeId` with scope inheritance.** Rejected: it recreates VS Code's flexibility at the cost of the type safety the project exists to exploit, and it is where god objects grow.
- **One global runtime trait.** Rejected: a single trait cannot express that a phone lacks a shell while a laptop has one.
