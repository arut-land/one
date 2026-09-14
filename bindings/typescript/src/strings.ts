import { FluentBundle, FluentResource } from "@fluent/bundle";
import { negotiateLanguages } from "@fluent/langneg";
// A type-only import: `./strings` is the subpath a webview loads without the
// wasm module behind it (ADR 0011), so nothing here may reach `@arut/ffi` at
// run time.
import type { ErrorArg } from "@arut/ffi";
import { catalog, locales } from "./generated/catalog";
import type { L10nBundle } from "./generated/l10n";

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

/** A handle that names its current error by Fluent id (ADR 0016). */
export interface ErrorSource {
  errorKey(): string | null;
  /** The error's arguments, each under the name its message interpolates. */
  errorArgs(): readonly ErrorArg[];
}

/**
 * The sentence for whatever error a handle is holding.
 *
 * Every argument arrives under the name the message interpolates -- Rust is
 * what knows that pairing (`product/i18n` validates it per variant), so no
 * surface and no binding keeps a table of its own and nothing is paired by
 * position.
 */
export function errorMessage(source: ErrorSource): string | null {
  const key = source.errorKey();
  if (key === null) return null;
  const args = source.errorArgs();
  if (args.length === 0) return strings.format(key);
  const values: Record<string, string> = {};
  for (const arg of args) values[arg.name] = arg.value;
  return strings.format(key, values);
}
