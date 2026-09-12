using Microsoft.Windows.ApplicationModel.Resources;

using Arut.Ffi;

namespace Arut.Surface.Windows;

// The core returns typed outcomes only (ADR 0016) and every sentence lives once
// in product/i18n as Fluent (ADR 0022). This file is only the mapping from a
// variant to its resource name in Strings/<lang>/Resources.resw, which
// `mise run i18n` generates; the lookup and the language come from the app's
// own ResourceLoader, so this surface localizes like any other WinUI app.
//
// Resource names match `message_key` on the same enum in
// `arut_feature_chat::errors`, with `-` replaced by `_`.
internal static class Strings
{
    private static readonly ResourceLoader Resources = new();

    private static string Localized(string name) => Resources.GetString(name);

    private static string Localized(string name, object argument) =>
        string.Format(Resources.GetString(name), argument);

    public static string ResourceName(NodeFailure failure) => failure switch
    {
        NodeFailure.Unreachable => "node_failure_unreachable",
        NodeFailure.TimedOut => "node_failure_timed_out",
        NodeFailure.Cancelled => "node_failure_cancelled",
        NodeFailure.Refused => "node_failure_refused",
        NodeFailure.Overloaded => "node_failure_overloaded",
        NodeFailure.Rejected => "node_failure_rejected",
        NodeFailure.Missing => "node_failure_missing",
        NodeFailure.Conflict => "node_failure_conflict",
        NodeFailure.Unsupported => "node_failure_unsupported",
        _ => "node_failure_internal",
    };

    public static string Describe(NodeFailure failure) => Localized(ResourceName(failure));

    public static string Describe(ComposerError error) => error switch
    {
        ComposerError.Node node => Describe(node.Field0),
        ComposerError.RevisionConflict conflict =>
            Localized("composer_error_revision_conflict", conflict.Current),
        ComposerError.AuthorityChanged moved =>
            Localized("composer_error_authority_changed", moved.CurrentEpoch),
        ComposerError.SnapshotMissing => Localized("composer_error_snapshot_missing"),
        ComposerError.OutcomeMissing => Localized("composer_error_outcome_missing"),
        ComposerError.ScopeMissing => Localized("composer_error_scope_missing"),
        ComposerError.ScopeMismatch => Localized("composer_error_scope_mismatch"),
        _ => Localized("node_failure_internal"),
    };

    public static string Describe(ChatError error) => error switch
    {
        ChatError.Node node => Describe(node.Field0),
        ChatError.NoConversation => Localized("chat_error_no_conversation"),
        ChatError.Cancelled => Localized("chat_error_cancelled"),
        ChatError.Draft draft => Describe(draft.Field0),
        _ => Localized("chat_error_chat_id_missing"),
    };
}
