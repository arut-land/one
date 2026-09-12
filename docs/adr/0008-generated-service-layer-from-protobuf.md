---
status: accepted
---

# The service layer is generated from Protobuf descriptors by a project-owned generator

Handwritten connection traits, procedure paths, and dispatch drift from the declared service. We keep the project-owned `prost_build::ServiceGenerator` that emits, per service, an object-safe trait, a client with direct (no encoding) and remote (encoded at the channel) targets, an erased router, canonical procedure names, and descriptors for all four streaming shapes. Protobuf files are discovered recursively; adding a package needs no Rust edits. Descriptors are the static input to the capability manifest, and the generator will also emit per-service availability types so no feature hand-writes a manifest lookup.

## Consequences

- Client and server procedure names cannot drift.
- Local composition is typed and serialization-free; remote dispatch serializes exactly once at the boundary.
- The generator is a substrate crate under a permissive license so it can be reused outside Arut.
