# Substrates

`watch` wraps Tokio watch for versioned, coalesced observation. `storage` defines
FactLog, BlobStore, and KeyValue with memory and directory implementations.
`authority` implements generic command acceptance and pure projection contracts.
These crates contain no product-specific rules. Identity and routing directories
will appear with their first implementations in later phases.
