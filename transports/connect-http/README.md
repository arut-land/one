# Connect HTTP

Unary Protobuf and server streams follow https://connectrpc.com/docs/protocol/.
Identity compression is supported. Unsupported compression flags are rejected.
The decoder accepts fragmented envelopes and limits each message to 8 MiB.
Errors preserve status codes and opaque details. The axum router owns framing,
not product dispatch. Request-streaming methods return Unimplemented.
