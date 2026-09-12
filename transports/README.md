# Transports

`memory` delegates to a registered byte channel in process. `connect-http`
implements Connect unary and server-stream framing over HTTP, including terminal
error envelopes and bounded frame decoding. `ipc` uses the same HTTP framing over
Unix sockets, avoiding a second protocol. Transports create no executors.
