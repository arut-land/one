---
status: accepted
amended-by: 0019 (transport encryption is iroh's QUIC; device key is the iroh endpoint key; at-rest sealed boxes and pairing remain as written)
---

# Device keys, linked-device root key, and end-to-end encryption from the first release

Retrofitting encryption changes envelope format, pairing, and store-and-forward at once, so we decided to do it before any of those ship. Each device generates an Ed25519 key on first launch. Pairing by QR or short code establishes a secure channel and transfers a per-person root key from which content keys derive. Every envelope between devices, and every backup held by the backend, is encrypted under keys the backend never holds. We use proven constructions (a Noise handshake for sessions, sealed boxes for stored envelopes) rather than our own.

## Consequences

- Losing every device without an account backup loses the data; this is accepted and stated to the person.
- Accounts later add root-key escrow under the account, following the linked-devices model messaging apps use.
- Pairing is the only path to trust; there is no "trust the LAN" mode.
