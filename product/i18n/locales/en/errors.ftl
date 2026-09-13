# Every user-facing sentence for a typed core failure (ADR 0016, ADR 0022).
#
# Message ids follow the enum they describe: a variant of `NodeFailure`,
# `ComposerError` or `ChatError` in `arut_feature_chat::errors` is
# `<enum-in-kebab-case>-<variant-in-kebab-case>`, and each variant field a
# surface shows becomes a Fluent argument of the same name in lowerCamelCase.
# `arut_feature_chat::errors::*::message_key` is the other half of that
# convention, and `tests/error_keys.rs` fails if the two ever disagree.
#
# Revisions and epochs are exact decimal identifiers, passed as text so every
# platform can display the full unsigned 64-bit range.

## NodeFailure -- why a call to the node produced no usable answer.

node-failure-unreachable = Arut can't reach your node right now.
node-failure-timed-out = Your node is taking too long to answer.
node-failure-cancelled = That request was cancelled before your node answered.
node-failure-refused = Your node refused that request.
node-failure-overloaded = Your node is too busy right now. Try again shortly.
node-failure-rejected = Your node couldn't accept that as it stands.
node-failure-missing = Your node says that no longer exists.
node-failure-conflict = Something else changed first. Try again.
node-failure-unsupported = Your node doesn't support that yet.
node-failure-internal = Something went wrong on your node.

## ComposerError -- why the draft in one composer scope is not what the node holds.

composer-error-revision-conflict = Someone else edited this draft first, so your edit didn't go through; it is now at revision { $current }.
composer-error-authority-changed = This conversation moved to a new authority (epoch { $currentEpoch }), so your edit didn't go through. Try again.
composer-error-snapshot-missing = Your node didn't send back the draft, so it may be out of sync.
composer-error-outcome-missing = Your node didn't say what happened to your edit.
composer-error-scope-missing = Your node didn't say which draft it meant.
composer-error-scope-mismatch = Your node answered about a different draft.

## ChatError -- why a chat could not accept what a person did.

chat-error-no-conversation = There's no conversation to send this to yet.
chat-error-cancelled = This conversation closed before your message could send.
chat-error-chat-id-missing = Your node started a conversation but didn't tell us its name.

## Connect-time transport statuses, before any scope exists to fail.

rpc-error-unavailable = The local node is unavailable.
rpc-error-cancelled = The request was cancelled.
rpc-error-rejected = The node could not accept this request.
rpc-error-timed-out = The node took too long to respond.
rpc-error-not-found = The requested conversation was not found.
rpc-error-already-exists = This item already exists on the node.
rpc-error-denied = Access to this node was denied.
rpc-error-exhausted = The node has no capacity for this request.
rpc-error-changed = The node changed before the request completed. Try again.
rpc-error-unsupported = This node does not support the request.
rpc-error-internal = The node encountered an internal error.

## NodeStartupFailure -- why `arut_runtime_local::child::ChildHost` could not
## give a session a working connection to the local node at all (ADR 0011).
## This is a separate enum from `NodeFailure` above: that one is why a call
## over an already-open connection failed, this is why the connection never
## came to be, so the two never share a variant name.

node-startup-failure-lease-held = Another running copy of your node already holds its lease.
node-startup-failure-socket-unreachable = Your node's socket exists, but nothing answered on it.
node-startup-failure-spawn-failed = Your node process could not be started.
