using Arut.Ffi;

namespace Arut.Surface.Windows;

// The core returns typed outcomes only (ADR 0016); this surface owns every
// user-facing string. Sentences are written from the person's side of the
// screen: what happened to what they were doing, not what the core did.
internal static class Strings
{
    public static string Describe(NodeFailure failure) => failure switch
    {
        NodeFailure.Unreachable => "Arut can't reach your node right now.",
        NodeFailure.TimedOut => "Your node is taking too long to answer.",
        NodeFailure.Cancelled => "That request was cancelled before your node answered.",
        NodeFailure.Refused => "Your node refused that request.",
        NodeFailure.Overloaded => "Your node is too busy right now. Try again shortly.",
        NodeFailure.Rejected => "Your node couldn't accept that as it stands.",
        NodeFailure.Missing => "Your node says that no longer exists.",
        NodeFailure.Conflict => "Something else changed first. Try again.",
        NodeFailure.Unsupported => "Your node doesn't support that yet.",
        NodeFailure.Internal => "Something went wrong on your node.",
        _ => "Something went wrong on your node.",
    };

    public static string Describe(ComposerError error) => error switch
    {
        ComposerError.Node node => Describe(node.Field0),
        ComposerError.RevisionConflict => "Someone else edited this draft first, so your edit didn't go through.",
        ComposerError.AuthorityChanged => "This conversation moved to a new authority, so your edit didn't go through. Try again.",
        ComposerError.SnapshotMissing => "Your node didn't send back the draft, so it may be out of sync.",
        ComposerError.OutcomeMissing => "Your node didn't say what happened to your edit.",
        ComposerError.ScopeMissing => "Your node didn't say which draft it meant.",
        ComposerError.ScopeMismatch => "Your node answered about a different draft.",
        _ => "Something went wrong with your draft.",
    };

    public static string Describe(ChatError error) => error switch
    {
        ChatError.Node node => Describe(node.Field0),
        ChatError.NoConversation => "There's no conversation to send this to yet.",
        ChatError.Cancelled => "This conversation closed before your message could send.",
        ChatError.Draft draft => Describe(draft.Field0),
        ChatError.ChatIdMissing => "Your node started a conversation but didn't tell us its name.",
        _ => "Something went wrong sending your message.",
    };
}
