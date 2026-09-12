---
status: accepted
---

# Product crates under FSL-1.1-Apache-2.0; substrate and generator crates under MIT or Apache-2.0

The project is built in public and must stay open, while the product should not be resold by competitors before it can be commercialized. We decided on the Functional Source License (FSL-1.1-Apache-2.0) for features, product, surfaces, and backend, which forbids competing use and converts to Apache-2.0 two years after each release, and dual MIT/Apache-2.0 for protocols, the RPC and generator crates, the reactive, storage, and transport substrates, and the BoltFFI wrapper, so those can be reused freely. The repository is public and the code is never delayed; only announcements are.

## Considered options

- **BUSL-1.1.** Rejected: same intent with a longer conversion and more legal text.
- **PolyForm Noncommercial.** Rejected: forbids non-competing commercial use, which conflicts with the open-source ethos.
- **AGPL.** Rejected: does not prevent a hosted competitor.
