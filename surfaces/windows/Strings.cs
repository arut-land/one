namespace Arut.Surface.Windows;

// Map Rust outcomes to the generated, localized Windows resource accessors.
internal static class Strings
{
    public static string Describe(NodeFailure failure) =>
        failure switch
        {
            NodeFailure.Unreachable => L10n.NodeFailureUnreachable(),
            NodeFailure.TimedOut => L10n.NodeFailureTimedOut(),
            NodeFailure.Cancelled => L10n.NodeFailureCancelled(),
            NodeFailure.Refused => L10n.NodeFailureRefused(),
            NodeFailure.Overloaded => L10n.NodeFailureOverloaded(),
            NodeFailure.Rejected => L10n.NodeFailureRejected(),
            NodeFailure.Missing => L10n.NodeFailureMissing(),
            NodeFailure.Conflict => L10n.NodeFailureConflict(),
            NodeFailure.Unsupported => L10n.NodeFailureUnsupported(),
            _ => L10n.NodeFailureInternal(),
        };

    public static string Describe(ComposerError error) =>
        error switch
        {
            ComposerError.Node node => Describe(node.Field0),
            ComposerError.RevisionConflict conflict => L10n.ComposerErrorRevisionConflict(
                conflict.Current.ToString(System.Globalization.CultureInfo.InvariantCulture)
            ),
            ComposerError.AuthorityChanged moved => L10n.ComposerErrorAuthorityChanged(
                moved.CurrentEpoch.ToString(System.Globalization.CultureInfo.InvariantCulture)
            ),
            ComposerError.SnapshotMissing => L10n.ComposerErrorSnapshotMissing(),
            ComposerError.OutcomeMissing => L10n.ComposerErrorOutcomeMissing(),
            ComposerError.ScopeMissing => L10n.ComposerErrorScopeMissing(),
            _ => L10n.ComposerErrorScopeMismatch(),
        };

    public static string Describe(ChatError error) =>
        error switch
        {
            ChatError.Node node => Describe(node.Field0),
            ChatError.NoConversation => L10n.ChatErrorNoConversation(),
            ChatError.Cancelled => L10n.ChatErrorCancelled(),
            ChatError.Draft draft => Describe(draft.Field0),
            _ => L10n.ChatErrorChatIdMissing(),
        };
}
