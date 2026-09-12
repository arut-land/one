# Local runtime

`arutd` hosts chat and composer services over Connect HTTP or a private Unix
socket. Directory storage contains immutable chat facts, raw blobs, and local
draft recovery values. ARUT_DATA selects the directory; ARUT_SOCKET selects IPC;
ARUT_ADDRESS selects TCP.

TokioSpawner receives an executor handle from the composition root. ChannelHost
provides scheduled RPC for foreign pollers. ChildHost starts arutd and awaits its
READY handshake. The final channel drop terminates the child and removes its
socket. No HTTP transport creates an executor.
