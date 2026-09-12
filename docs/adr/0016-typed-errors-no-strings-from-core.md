---
status: accepted
---

# The core returns typed errors and outcomes; surfaces own every user-facing string

Internationalization is deferred and the first release is English only, which makes returning English strings from Rust tempting. We decided the core never returns user-facing text: errors, unavailable reasons, and stale outcomes are enums with typed payloads, and each surface maps them to strings in its own resource system. This keeps availability projections typed, keeps error handling testable, and means adding a language later touches surfaces only.

## Consequences

- `error: String` fields in projections are removed in favor of typed variants.
- Each surface has a small, complete mapping from core outcomes to platform strings.
