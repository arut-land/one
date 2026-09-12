import { FluentBundle, FluentResource, type FluentVariable } from "@fluent/bundle";
import { negotiateLanguages } from "@fluent/langneg";
import { NodeFailure, type ChatError, type ComposerError } from "@arut/ffi";

// The core returns typed outcomes only (ADR 0016) and every sentence lives once
// in product/i18n as Fluent (ADR 0022). This module is the surfaces' half of
// that: it maps a typed error to a message id, loads the same .ftl files the
// generator copied next to each surface, and formats them with @fluent/bundle.
// Locale choice stays with the host -- a browser negotiates from
// navigator.languages, the VS Code extension host from vscode.env.language.

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

/**
 * The sentence for one message id, from the first negotiated locale that has
 * it. An id no locale defines comes back as itself, which is visible on screen
 * and impossible to mistake for copy.
 */
export function message(key: string, args?: Record<string, FluentVariable>): string {
  for (const bundle of bundles) {
    const found = bundle.getMessage(key);
    if (!found?.value) continue;
    const errors: Error[] = [];
    const text = bundle.formatPattern(found.value, args, errors);
    if (errors.length === 0) return text;
  }
  return key;
}

/**
 * The message id for a `NodeFailure`, matching `NodeFailure::message_key` in
 * `arut_feature_chat::errors`. The two are checked against each other by
 * `arut-i18n`'s key test, which reads the same `.ftl` files this does.
 */
function nodeFailureKey(failure: NodeFailure): string {
  switch (failure) {
    case NodeFailure.Unreachable: return "node-failure-unreachable";
    case NodeFailure.TimedOut: return "node-failure-timed-out";
    case NodeFailure.Cancelled: return "node-failure-cancelled";
    case NodeFailure.Refused: return "node-failure-refused";
    case NodeFailure.Overloaded: return "node-failure-overloaded";
    case NodeFailure.Rejected: return "node-failure-rejected";
    case NodeFailure.Missing: return "node-failure-missing";
    case NodeFailure.Conflict: return "node-failure-conflict";
    case NodeFailure.Unsupported: return "node-failure-unsupported";
    case NodeFailure.Internal: return "node-failure-internal";
    default: return "node-failure-internal";
  }
}

/** One sentence for every `ComposerError` variant, payload included. */
export function describeComposerError(error: ComposerError): string {
  switch (error.tag) {
    case "Node":
      return message(nodeFailureKey(error.value0));
    case "RevisionConflict":
      return message("composer-error-revision-conflict", { current: Number(error.current) });
    case "AuthorityChanged":
      return message("composer-error-authority-changed", { currentEpoch: Number(error.currentEpoch) });
    case "SnapshotMissing":
      return message("composer-error-snapshot-missing");
    case "OutcomeMissing":
      return message("composer-error-outcome-missing");
    case "ScopeMissing":
      return message("composer-error-scope-missing");
    case "ScopeMismatch":
      return message("composer-error-scope-mismatch");
    default:
      return message("node-failure-internal");
  }
}

/** One sentence for every `ChatError` variant. */
export function describeChatError(error: ChatError): string {
  switch (error.tag) {
    case "Node":
      return message(nodeFailureKey(error.value0));
    case "NoConversation":
      return message("chat-error-no-conversation");
    case "Cancelled":
      return message("chat-error-cancelled");
    case "Draft":
      return describeComposerError(error.value0);
    case "ChatIdMissing":
      return message("chat-error-chat-id-missing");
    default:
      return message("node-failure-internal");
  }
}
