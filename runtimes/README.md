# Runtimes

Composition roots and host-specific port implementations. `local` supplies the
Tokio Spawner, scheduled RPC channels, and the `arutd` binary. Executors belong
here; transports run on the caller's executor.
