import { FluentBundle, FluentResource, type FluentVariable } from "@fluent/bundle";
import { negotiateLanguages } from "@fluent/langneg";
import { NodeFailure, type ChatError, type ComposerError } from "@arut/ffi";
import { t, type L10nBundle } from "./generated/l10n";

// The core returns typed outcomes only (ADR 0016) and every sentence lives once
// in product/i18n as Fluent (ADR 0022). This module loads the same .ftl files
// the generator copied beside each surface and formats them with
// @fluent/bundle; the message ids are never written here, they come from the
// generated `t` in ./generated/l10n. Locale choice stays with the host -- a
// browser negotiates from navigator.languages, the VS Code extension host from
// vscode.env.language.

export { t, keys, type L10nBundle, type MessageKey } from "./generated/l10n";

/** What the generator writes beside the `.ftl` tree it copies out. */
interface Manifest {
  locales: string[];
  files: string[];
}

/** Where and how to read the generated Fluent tree. */
export interface StringsSource {
  /**
   * The directory holding `locales.json` and `<lang>/<file>.ftl`. Ignored when
   * `read` is supplied. Defaults to the web surface's served copy.
   */
  baseUrl?: string;
  /** Languages best first. Defaults to `navigator.languages`. */
  preferred?: readonly string[];
  /**
   * Read one file of the tree by its path within it (`locales.json`,
   * `en/errors.ftl`). A host without `fetch` -- the VS Code extension host
   * reading its own resource URIs, say -- supplies this instead of `baseUrl`.
   */
  read?: (file: string) => Promise<string>;
}

/** The negotiated bundles, best locale first. Empty until `loadStrings`. */
let bundles: FluentBundle[] = [];

/**
 * The loaded string source, as the generated accessors want it.
 *
 * Pass it as the first argument to any `t.*`: `t.actionSend(strings)`.
 */
export const strings: L10nBundle = {
  format(key: string, args?: Record<string, FluentVariable>): string {
    for (const bundle of bundles) {
      const found = bundle.getMessage(key);
      if (!found?.value) continue;
      const errors: Error[] = [];
      const text = bundle.formatPattern(found.value, args, errors);
      if (errors.length === 0) return text;
    }
    // A key no locale defines comes back as itself, which is visible on screen
    // and impossible to mistake for copy.
    return key;
  },
};

/**
 * Load the string source and negotiate the person's locale against it.
 *
 * Call it once during a surface's bootstrap, before the first render: every
 * `describe*` below is synchronous, and until this resolves they answer with
 * the message id rather than a sentence.
 *
 * Returns the negotiated locales, best first.
 */
export async function loadStrings(source: StringsSource = {}): Promise<readonly string[]> {
  const base = source.baseUrl ?? "/locales";
  const read = source.read ?? (async (file: string) => {
    const response = await fetch(`${base}/${file}`);
    if (!response.ok) throw new Error(`${base}/${file}: ${response.status} ${response.statusText}`);
    return response.text();
  });
  const manifest = JSON.parse(await read("locales.json")) as Manifest;
  const preferred = source.preferred
    ?? (typeof navigator === "undefined" ? [] : navigator.languages);
  const chosen = negotiateLanguages([...preferred], manifest.locales, {
    defaultLocale: manifest.locales[0],
    strategy: "filtering",
  });
  bundles = await Promise.all(chosen.map(async locale => {
    // Fluent isolates placeables with U+2068/U+2069 by default, which show up
    // as stray characters anywhere the string is not rendered as bidi text.
    const bundle = new FluentBundle(locale, { useIsolating: false });
    for (const file of manifest.files) {
      bundle.addResource(new FluentResource(await read(`${locale}/${file}`)));
    }
    return bundle;
  }));
  return chosen;
}

/** One sentence for every `NodeFailure` variant. */
function describeNodeFailure(failure: NodeFailure): string {
  switch (failure) {
    case NodeFailure.Unreachable: return t.nodeFailureUnreachable(strings);
    case NodeFailure.TimedOut: return t.nodeFailureTimedOut(strings);
    case NodeFailure.Cancelled: return t.nodeFailureCancelled(strings);
    case NodeFailure.Refused: return t.nodeFailureRefused(strings);
    case NodeFailure.Overloaded: return t.nodeFailureOverloaded(strings);
    case NodeFailure.Rejected: return t.nodeFailureRejected(strings);
    case NodeFailure.Missing: return t.nodeFailureMissing(strings);
    case NodeFailure.Conflict: return t.nodeFailureConflict(strings);
    case NodeFailure.Unsupported: return t.nodeFailureUnsupported(strings);
    case NodeFailure.Internal: return t.nodeFailureInternal(strings);
    default: return t.nodeFailureInternal(strings);
  }
}

/** One sentence for every `ComposerError` variant, payload included. */
export function describeComposerError(error: ComposerError): string {
  switch (error.tag) {
    case "Node":
      return describeNodeFailure(error.value0);
    case "RevisionConflict":
      return t.composerErrorRevisionConflict(strings, { current: Number(error.current) });
    case "AuthorityChanged":
      return t.composerErrorAuthorityChanged(strings, { currentEpoch: Number(error.currentEpoch) });
    case "SnapshotMissing":
      return t.composerErrorSnapshotMissing(strings);
    case "OutcomeMissing":
      return t.composerErrorOutcomeMissing(strings);
    case "ScopeMissing":
      return t.composerErrorScopeMissing(strings);
    case "ScopeMismatch":
      return t.composerErrorScopeMismatch(strings);
    default:
      return t.nodeFailureInternal(strings);
  }
}

/** One sentence for every `ChatError` variant. */
export function describeChatError(error: ChatError): string {
  switch (error.tag) {
    case "Node":
      return describeNodeFailure(error.value0);
    case "NoConversation":
      return t.chatErrorNoConversation(strings);
    case "Cancelled":
      return t.chatErrorCancelled(strings);
    case "Draft":
      return describeComposerError(error.value0);
    case "ChatIdMissing":
      return t.chatErrorChatIdMissing(strings);
    default:
      return t.nodeFailureInternal(strings);
  }
}
