---
status: accepted
amends: 0016
---

# User-facing strings have one Fluent source and reach each surface as native resources

The first typed-error pass produced the same string table five times, once per surface, which is the duplication this project exists to avoid. We decided that every user-facing string lives once in Fluent (`.ftl`) files under `product/i18n`, and that a generator turns them into each platform's native resource format: `Localizable.xcstrings` for Apple, `values-<lang>/strings.xml` for Android, `.resw` for Windows. Those surfaces use their platform's own localization API with no change in idiom. Rust-owned surfaces load the Fluent bundle directly; web and editor surfaces load the same files with `@fluent/bundle`. Typed errors map to message keys by a naming convention, with variant fields as Fluent arguments, and a test derives the required keys from the enums so no locale can miss one.

ADR 0016 stands: the core still emits typed values and no text. What changes is that surfaces no longer each author their strings; they render one shared source through their native mechanism.

## Considered options

- **A `translate(key, args)` function exposed over BoltFFI.** Rejected: one FFI call per rendered string, formatting on the wrong side of the boundary, bypasses per-app language settings and platform localization tooling, and forces locale state into the core.
- **Per-surface string files as the source.** Rejected: five sources drift; the typed-error table already did.
- **ICU MessageFormat as the source.** Rejected: no platform consumes it natively either, and Fluent's selectors read better and have first-class tooling in Pontoon and Weblate.

## Consequences

- Locale selection stays platform-owned; the core never knows the locale.
- Strings never cross FFI; runtime cost on native surfaces is zero.
- v1 ships English only with the machinery in place; a language is one file plus a regenerate.
- Complex Fluent features that a target format cannot express (nested selectors) are rejected by the generator with a clear error rather than degraded silently.
- Keys are never typed by hand. The generator also emits typed accessors per message for every consumer: a Rust `Message` enum with data-carrying variants, `L10n` in Swift, Kotlin, and C#, and a typed module in TypeScript, with argument types inferred from Fluent usage (plural selector or `NUMBER()` is a number, `DATETIME()` a date, otherwise a string) and overridable by a comment. Error enums map to messages through a compile-time derive that reads the Fluent source, so a variant without a message fails to compile. Every locale must define the same key set or generation fails.

## Amendment (2026-09-14)

Typed accessors are generated only where the platform lacks a checked accessor of its own. Android uses `R.string` and `R.plurals`, so no Kotlin is generated and argument typing there falls back to the platform's `vararg Any`. Swift and C# get key constants over `String(localized:)` and `ResourceLoader`, C# keeping a generated `Quantity()` because `.resw` has no plural form. TypeScript gets a key union, an argument map, and one `t()`. The `.ftl` copies beside the browser and editor surfaces are replaced by one generated `catalog.ts` module, and `arut-dev check` fails an `x:Uid` in the WinUI XAML that the generated `.resw` does not define.
