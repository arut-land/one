# Arut Product Requirements

Status: target requirements, not a claim that these capabilities are implemented. Current status is in `docs/ROADMAP.md`; vocabulary follows `CONTEXT.md`.

## What Arut is

Arut is a personal AI agent presented as chat, reachable from every device a person owns, and executed on whichever of their nodes they choose. A conversation started on a laptop can be followed and continued from a phone. A task can run on a desktop while the person watches it from a watch. Nothing requires a server; a server makes more of it convenient.

Arut is not a coding tool. Coding is one harness among many. The product is the substrate that makes any harness feel native everywhere: synchronization, authority, capability awareness, and platform fidelity.

## Who it is for

First users are people who care about how software feels: performance, platform-native behavior, privacy, and open source. They will judge each surface against the best app on that platform, not against other AI tools.

Later users include teams and organizations, which adds accounts, sharing, permissions, and on-premises deployment. Nothing in v1 may make those harder.

## Principles

1. **Native, not uniform.** Every surface looks and behaves as if the platform's own developers and designers built it. No paradigm is forced across platforms. Gestures, navigation, theming, accessibility, keyboard conventions, and lifecycle all follow the host.
2. **One product, one truth.** Conversations, drafts, availability, and operations are the same everywhere. A change on one device appears on the others without the person doing anything.
3. **Local first, backend optional.** Everything works with no backend. The backend adds pairing across networks, relay, and encrypted backup. It never becomes the source of truth.
4. **Execution is a choice.** Where a conversation runs is picked when it starts and can be changed deliberately. Changing how a device reaches a node never changes where execution happens.
5. **Private by construction.** The backend cannot read content. Telemetry never carries content without explicit consent.
6. **Capabilities are reported, not assumed.** Every node says what it can do right now. Surfaces show availability and reasons instead of failing.
7. **Less code per feature.** A feature is one Rust crate. Bindings and surfaces gain it without hand-written glue.

## Version 1

The daily driver. Three surfaces, one feature set, real sync.

### Surfaces

- Linux desktop, GTK4 in Rust, adapting to the running desktop environment.
- macOS, SwiftUI.
- Android, Jetpack Compose.

Other existing surfaces are outside the v1 bar. Linux and TypeScript builds are verified locally; native Apple, Android, and Windows builds require their platform toolchains.

### Capabilities

- Multiple conversations per workspace, several running at once.
- A default workspace per node; workspaces are not shown in v1 UI.
- Streamed model responses, visible live on every paired device.
- A composer per conversation and a pending composer for new conversations. Drafts carry text and small attachments with previews, synchronized across devices, last writer wins.
- A node picker on the empty composer: choose which paired node will run the conversation. Phones can run conversations themselves.
- Pairing between a person's devices by QR code or short code.
- Two routes: direct on the local network, and relay through the backend. Iroh owns direct/relay selection and failover (ADR 0019).
- End-to-end encryption between paired devices. Backups through the backend that the backend cannot read.
- One model provider through an OpenAI-compatible endpoint, with the person's own key stored in the executing node's platform keychain.
- Peer-assisted delivery: a command sent to an unreachable node is carried by any paired device or the backend and delivered when the node returns.

### Explicitly out of v1

- Tools, approvals, and any harness beyond a model call.
- Hand-off of a running conversation to another node.
- Rich text in the composer.
- Accounts, sign-in, key escrow, push notifications.
- Additional translations; the localization machinery is implemented, but v1 is English only.
- iOS, Windows, VS Code, JetBrains, browser extensions, terminal, watches.
- Multi-user sharing.

### Scenarios v1 must satisfy

**Follow a conversation from another device.** Start a conversation on Linux. Open Android. The conversation is in the list, the transcript is complete, and a response still streaming on Linux is streaming on Android.

**Continue a draft elsewhere.** Type half a message on Android, put the phone down, open macOS. The draft is in the composer. Attach an image on macOS; its preview appears on Android.

**Run from a phone.** With no other device online, start a conversation on Android. It responds. Later, when the laptop is on, the conversation appears there with the phone as its node.

**Choose where it runs.** On the phone, pick the laptop in the node picker, send. The laptop executes; the phone shows the stream.

**Lose connectivity to the executing node.** Three devices: laptop A in another room, laptop B and a phone in hand. A conversation runs on A. The phone loses its path to A and sends a follow-up. Laptop B, paired and nearby, receives the follow-up. The phone is switched off. When A returns, B delivers the follow-up, A executes once, and the outcome reaches every device. If the follow-up depended on a state that changed meanwhile, the person sees a typed superseded result, not a duplicate or a silent drop.

**Parallel conversations.** While one conversation is generating, start another. Both stream. The UI stays responsive.

**Restore.** Reinstall on one device, pair it, restore from the backend backup. Every conversation and draft returns. The backend never had the content in the clear.

### Non-functional requirements

- **Responsiveness.** No surface blocks its UI thread on the core. Input latency in the composer is the platform's own.
- **Startup.** A surface shows the last state before any network activity.
- **Offline.** Every surface is usable offline for reading, drafting, and queuing commands.
- **Delivery.** At-least-once delivery with exactly-once effect, enforced by command identity at the authority.
- **Version skew.** A surface two minor versions behind a node still works; further apart, it says so with a typed reason.
- **Privacy.** Content never leaves a person's devices unencrypted. Telemetry is off until configured.
- **Accessibility.** Each surface meets its platform's accessibility guidance.

### Success criteria for v1

- The author uses it daily on Linux and Android for real conversations.
- Every scenario above passes on real devices.
- A new projection reaches all three surfaces without a hand-written binding.
- A second transport is added without touching a feature crate.

## After v1

Ordered by dependency, detailed in `docs/ROADMAP.md`.

1. **Hand-off.** Checkpoints, snapshots, resume on another node, explicit first, automatic later.
2. **Harness.** Tools, approvals from any surface, first external harness through an open protocol.
3. **Backend accounts.** Sign-in, key escrow, push notifications, cloud nodes as paired devices.
4. **Surfaces.** iOS and iPadOS, Windows, VS Code, JetBrains, browser extensions, terminal, then watches and messaging integrations.
5. **Routes.** Browser relay reach, Bluetooth if needed, and per-feature preferences above iroh. Connection probing and failover belong to iroh (ADR 0019).
6. **Organizations.** Sharing, permissions, tenant policy, on-premises backend.

## Business constraints

- Open source, built in public. Announcements lag the code; the code does not lag.
- Product crates under FSL-1.1-Apache-2.0. Substrate and generator crates under MIT or Apache-2.0.
- A deployable pairing/backup service alongside the iroh relay; no backend is implemented yet.
- Solo maintainer. Every design choice is weighed against maintenance cost first.
