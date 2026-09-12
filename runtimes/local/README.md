# Local runtime

`arutd` hosts chat and composer services. `TokioSpawner` receives an executor
handle from the root. `ChannelHost` supplies a scheduled channel to a session.
Unix-socket child-process hosting is completed with the IPC transport in step 5.
The current checkpoint persistence is replaced by storage ports in step 6.

ChildHost starts arutd, waits for its READY handshake, and returns a scheduled
Unix-socket channel. The final channel drop terminates the child and removes its
socket. Set ARUT_SOCKET for IPC, ARUT_ADDRESS for TCP, and ARUT_DATA for storage.
