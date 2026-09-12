# Arut

Arut is one product presented through native surfaces on every device a person owns, backed by execution that runs on whichever of their nodes they choose. This glossary is the language of that product. Implementation vocabulary lives in `docs/GLOSSARY.md`.

## Language

### Places where things run

**Node**:
A device or server running the Arut core that can hold conversations and execute operations. A phone, a laptop, and a cloud machine are each a node.
_Avoid_: host, machine, runtime, execution node, execution host, device (when the core is what matters)

**Workspace**:
A named context on a node that holds the resources, connectors, and settings its conversations may use. Every node has a default workspace.
_Avoid_: project, folder, repo

**Surface**:
Anything that presents Arut to a person or invokes it on their behalf: a native app, an editor extension, a browser extension, a terminal, a chat integration.
_Avoid_: client, frontend, UI, app (when the distinction from a node matters)

**Backend**:
The optional service that pairs devices, relays envelopes between them, and stores encrypted backups it cannot read. It is never the source of truth for a conversation.
_Avoid_: server, cloud (which is a node, not the backend), control plane

### Conversations

**Conversation**:
A durable exchange between a person and Arut with an ordered transcript. It is created on exactly one node, which becomes its authority.
_Avoid_: chat, thread, session

**Operation**:
One unit of work running inside a conversation, such as a model turn or a tool call. Operations can run in parallel across conversations.
_Avoid_: task, job, run, turn

**Composer**:
The place where a draft is written for a conversation. A conversation has one composer; before a conversation exists, the pending composer holds the draft that will start it.
_Avoid_: input, editor, text box

**Draft**:
The unsent text and attachments in a composer. A draft is shared across a person's devices but is not part of the transcript.
_Avoid_: message (until sent), input state

**Attachment**:
A file referenced by a draft or a message, stored by content and carried between nodes alongside the conversation.
_Avoid_: upload, file (when the relationship to a message matters)

**Message**:
One entry in a conversation's transcript, from the person or from Arut.
_Avoid_: chat message, post

### Authority and truth

**Authority**:
The single node that orders a conversation's facts and accepts its commands. There is exactly one authority per conversation at any time.
_Avoid_: owner, primary, master, leader

**Authority epoch**:
A generation number that increases each time a conversation's authority changes. A node holding an old epoch cannot commit.
_Avoid_: term, generation, version

**Fact**:
Something the authority has accepted and ordered: a message, an operation's start or end, an approval, an authority change. Facts are the durable truth of a conversation.
_Avoid_: event, record, entry, log item

**Command**:
A request to change a conversation, carrying a stable identity so it can be retried, carried, or delivered twice without executing twice.
_Avoid_: request, action, mutation, intent (which is the surface-side wish before it becomes a command)

**Command ID**:
The stable identity of a command, chosen by the surface that created it and kept through every retry and carrier.
_Avoid_: request ID, idempotency key

**Revision**:
The count of accepted changes to one scope, such as a composer's draft. Commands that depend on a current value name the revision they expect.
_Avoid_: version, sequence (which orders facts across a conversation)

**Projection**:
A bounded view of facts shaped for a surface to render, such as a transcript, a conversation list, or a composer state. Projections are rebuilt from facts and never authored directly.
_Avoid_: state, model, view model, store

**Hand-off**:
Moving a conversation's authority from one node to another through a checkpoint. Changing which route reaches a node is not a hand-off.
_Avoid_: migration (as a user-facing word), transfer, failover (which is a hand-off without a person asking)

**Checkpoint**:
A portable capture of a conversation's facts and durable state that a new authority can resume from.
_Avoid_: snapshot (which is a projection accelerator, not a hand-off artifact), backup (which is a checkpoint held by the backend)

### Devices and trust

**Device key**:
The identity of one device, generated on first launch and never shared. Everything a device says is signed by it.
_Avoid_: device ID (as a security concept), certificate

**Pairing**:
The act of two devices of one person proving they belong together, by QR code or short code, after which they may exchange envelopes.
_Avoid_: linking, login, connecting

**Root key**:
The secret shared among a person's paired devices from which all content keys derive. Without it, backups and envelopes cannot be read.
_Avoid_: master key, account key

**Envelope**:
A signed and encrypted unit carried between devices or through the backend. The backend can route an envelope but not read it.
_Avoid_: packet, message (which is a transcript entry), payload

**Carrier**:
Any paired device or the backend holding an envelope on behalf of another node until that node is reachable.
_Avoid_: relay (which is one carrier: the backend), proxy, queue

### Reaching things

**Route**:
A way for a surface or node to reach another node: in-process, local pipe, LAN, backend relay, peer connection. Changing the route never changes which node executes.
_Avoid_: transport (the implementation vocabulary word), connection, link

**Session**:
One surface's live attachment to a node, sharing identity, negotiated capabilities, and route across every conversation it shows.
_Avoid_: connection, chat, conversation

**Capability**:
Something a node can currently do, as it reports it: a service version, an installed tool, a permission, a limit. Capabilities are reported, never assumed.
_Avoid_: feature (which is what the product offers), permission (which is one kind of capability)

**Availability**:
A surface-facing answer to "can this be done here right now, and if not, why", derived from capabilities.
_Avoid_: enabled, supported, feature flag

**Manifest**:
The set of capabilities a node reports to a session when they connect.
_Avoid_: registry, catalog

### Doing the work

**Harness**:
The pluggable part of a node that turns a conversation and its workspace into operations: choosing models, running tools, asking for approvals.
_Avoid_: agent, engine, runtime, orchestrator

**Provider**:
An outbound dependency a harness or feature uses, such as a model API, a search service, or a storage service.
_Avoid_: integration (which may also be a surface), vendor, backend

**Approval**:
A decision a person makes about an operation that a harness has paused, accepted from whichever surface answers first.
_Avoid_: permission prompt, confirmation
