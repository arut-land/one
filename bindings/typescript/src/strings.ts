import { NodeFailure, type ChatError, type ComposerError } from "@arut/ffi";

// The core returns typed outcomes only (ADR 0016); every surface maps them to
// its own strings. This is the mapping for surfaces built on this package
// (web, VS Code). Sentences are written from the person's side of the
// screen: what happened to what they were doing, not what the core did.

function describeNodeFailure(failure: NodeFailure): string {
  switch (failure) {
    case NodeFailure.Unreachable:
      return "Arut can't reach your node right now.";
    case NodeFailure.TimedOut:
      return "Your node is taking too long to answer.";
    case NodeFailure.Cancelled:
      return "That request was cancelled before your node answered.";
    case NodeFailure.Refused:
      return "Your node refused that request.";
    case NodeFailure.Overloaded:
      return "Your node is too busy right now. Try again shortly.";
    case NodeFailure.Rejected:
      return "Your node couldn't accept that as it stands.";
    case NodeFailure.Missing:
      return "Your node says that no longer exists.";
    case NodeFailure.Conflict:
      return "Something else changed first. Try again.";
    case NodeFailure.Unsupported:
      return "Your node doesn't support that yet.";
    case NodeFailure.Internal:
      return "Something went wrong on your node.";
    default:
      return "Something went wrong on your node.";
  }
}

/** One English sentence for every `ComposerError` variant. */
export function describeComposerError(error: ComposerError): string {
  switch (error.tag) {
    case "Node":
      return describeNodeFailure(error.value0);
    case "RevisionConflict":
      return "Someone else edited this draft first, so your edit didn't go through.";
    case "AuthorityChanged":
      return "This conversation moved to a new authority, so your edit didn't go through. Try again.";
    case "SnapshotMissing":
      return "Your node didn't send back the draft, so it may be out of sync.";
    case "OutcomeMissing":
      return "Your node didn't say what happened to your edit.";
    case "ScopeMissing":
      return "Your node didn't say which draft it meant.";
    case "ScopeMismatch":
      return "Your node answered about a different draft.";
    default:
      return "Something went wrong with your draft.";
  }
}

/** One English sentence for every `ChatError` variant. */
export function describeChatError(error: ChatError): string {
  switch (error.tag) {
    case "Node":
      return describeNodeFailure(error.value0);
    case "NoConversation":
      return "There's no conversation to send this to yet.";
    case "Cancelled":
      return "This conversation closed before your message could send.";
    case "Draft":
      return describeComposerError(error.value0);
    case "ChatIdMissing":
      return "Your node started a conversation but didn't tell us its name.";
    default:
      return "Something went wrong sending your message.";
  }
}
