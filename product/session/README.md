# Session

Typed Node, Workspace, Conversation, and Operation scopes hold their parents.
ChatRuntime is a bundle of narrow service capabilities. A session constructs its
default node and workspace; configuration falls back from workspace to node.
The conversation registry uses a weak registration callback, with no self pointer
or OnceLock. Selection belongs to the session's surface-facing navigation API.
