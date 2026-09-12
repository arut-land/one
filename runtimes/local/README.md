# Local runtime

`arutd` hosts chat and composer services. `TokioSpawner` receives an executor
handle from the root. `ChannelHost` supplies a scheduled channel to a session.
Unix-socket child-process hosting is completed with the IPC transport in step 5.
The current checkpoint persistence is replaced by storage ports in step 6.
