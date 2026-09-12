# Features

A feature owns product meaning. Keep its portable domain behavior, product-facing state, intents, ports, and tests together in one crate until a proven dependency requires a split.

Feature modules have distinct roles:

- `domain` contains portable rules and algorithms.
- `product` contains UI-facing state, commands, and narrow execution ports.
- `service` implements feature meaning behind a generated protocol service trait.

Generated service clients and routers remove per-feature transport mapping. Protocols, RPC channels, generic transports, runtime capabilities, bindings, and native views stay in their role-specific directories. Features do not select endpoints, routes, runtimes, or concrete transports.

Not every feature needs every role. Add state, protocols, streams, providers, or runtime adapters only when its behavior crosses that boundary.
