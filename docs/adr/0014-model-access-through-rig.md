---
status: accepted
---

# Model access goes through the `ModelProvider` port, implemented with `rig`

The first release needs one real model provider and later releases need many, with the harness deciding which to use. We decided the core depends on a `ModelProvider` port that takes a typed request and returns a token stream, and that the first implementation uses the `rig` crate against an OpenAI-compatible endpoint. Provider keys live in the executing node's platform keychain; a backend broker comes with accounts so a phone can run on a cloud node without holding a key.

## Consequences

- Adding a provider is one crate and no feature change.
- `rig` depends on `reqwest`, which cannot run inside the BoltFFI wasm core; browser-side execution must go through host callbacks or a node.
