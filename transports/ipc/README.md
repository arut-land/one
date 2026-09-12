# IPC

Connect over HTTP on a Unix socket. The local daemon creates a private socket;
the channel uses reqwest's Unix-socket connector and shares the Connect codec.
