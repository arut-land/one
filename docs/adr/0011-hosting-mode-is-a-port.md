---
status: accepted
---

# Where the core runs is a port chosen per platform by the composition root

Each platform has its own recommendation for offloading work: a child process on desktops, a foreground service on Android, in-process only on iOS, the extension host in VS Code, a worker in browsers. We decided that hosting is a `Host` port with four modes (in-process thread, child process, system service, remote node) and that the composition root of each surface selects one. Defaults: Linux and Windows spawn a supervised child process; macOS spawns a child now and can move to a login item later; Android runs the core in a foreground service; iOS runs in-process; VS Code runs the core in the extension host with a webview UI over a transport; web and browser extensions run wasm in the page or a worker. Nothing above the port knows which mode it received.

## Consequences

- The daemon binary (`arutd`) exists from the first release on desktops.
- Promoting a child process to an always-on service is a composition-root change, not an architecture change.
