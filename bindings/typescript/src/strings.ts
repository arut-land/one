import { FluentBundle, FluentResource } from "@fluent/bundle";
import { negotiateLanguages } from "@fluent/langneg";
import { catalog, locales } from "./generated/catalog";
import type { L10nBundle, MessageKey } from "./generated/l10n";

// The core returns typed outcomes only (ADR 0016) and every sentence lives
// once in product/i18n as Fluent (ADR 0022). The generator emits that source
// as ./generated/catalog, so a surface holds no locale files of its own and
// needs no fetch: negotiation is synchronous, and no composition root awaits a
// string before its first render.

export { t, type Args, type L10nBundle, type MessageKey } from "./generated/l10n";
export { locales } from "./generated/catalog";

type Locale = (typeof locales)[number];

function load(preferred: readonly string[]): FluentBundle[] {
  const chosen = negotiateLanguages([...preferred], [...locales], {
    defaultLocale: locales[0],
    strategy: "filtering",
  });
  return chosen
    .filter((locale): locale is Locale => locale in catalog)
    .map(locale => {
      // Fluent isolates placeables with U+2068/U+2069 by default, which show up
      // as stray characters anywhere the string is not rendered as bidi text.
      const bundle = new FluentBundle(locale, { useIsolating: false });
      bundle.addResource(new FluentResource(catalog[locale]));
      return bundle;
    });
}

let bundles = load(globalThis.navigator?.languages ?? []);

/**
 * Negotiate the person's languages against the catalog, best first.
 *
 * A surface whose host names the language itself -- the VS Code extension host
 * reading `vscode.env.language` -- calls this during activation; a browser page
 * needs no call, because `navigator.languages` is negotiated on load.
 */
export function selectLocale(preferred: readonly string[]): readonly string[] {
  bundles = load(preferred);
  return bundles.map(bundle => bundle.locales[0] ?? locales[0]);
}

/**
 * The loaded string source, as the generated accessor wants it.
 *
 * Pass it as the first argument to `t`: `t(strings, "action-send")`.
 */
export const strings: L10nBundle = {
  format(key: string, args?: Record<string, string | number>): string {
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
 * The messages whose arguments the core sends positionally, in message order.
 *
 * Every other error message takes none, so this is the whole of what a surface
 * has to know about error arguments -- the ids themselves come from the core.
 */
const ERROR_ARGUMENTS = new Map<string, readonly string[]>(
  Object.entries({
    "composer-error-authority-changed": ["currentEpoch"],
    "composer-error-revision-conflict": ["current"],
  } satisfies Partial<Record<MessageKey, readonly string[]>>),
);

/** A handle that names its current error by Fluent id (ADR 0016). */
export interface ErrorSource {
  errorKey(): string | null;
  errorArgs(): string[];
}

/** The sentence for whatever error a chat or composer handle is holding. */
export function errorMessage(source: ErrorSource): string | null {
  const key = source.errorKey();
  if (key === null) return null;
  const names = ERROR_ARGUMENTS.get(key) ?? [];
  const values = source.errorArgs();
  return strings.format(key, Object.fromEntries(names.map((name, index) => [name, values[index] ?? ""])));
}
