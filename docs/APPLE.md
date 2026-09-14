# Apple app

The Apple app uses SwiftUI over `ArutBindings`. The composition root creates the product session; views observe the Rust projections and invoke their commands. The current FFI factory uses memory ports. Conversations survive switching within the session, but restarting the app does not persist them. The native daemon's redb store is not connected to this app yet.

## Build and run

Use Xcode 26 or newer to compile the Liquid Glass APIs. The app targets macOS 14 and iOS 17, the floor SwiftUI's Observation needs. Native Liquid Glass controls and `safeAreaBar` are enabled on macOS/iOS 26; earlier systems use bordered controls and `safeAreaInset`. Swift integration tests use the Swift Testing framework bundled with Xcode. The optional macOS UI-test target requires macOS 14 or later.

```sh
mise run check:apple
open surfaces/apple/DerivedData/Build/Products/Debug/ArutMac.app
```

`check:apple` installs the Apple Rust targets, packages the XCFramework, generates the Xcode project, builds ArutMac without distribution signing, and runs the Swift binding tests. CI runs this task on pull requests and release tags.

For an incremental Swift-only build, after generating the framework and project:

```sh
mise run --skip-deps surface:macos
```

Regenerate `surfaces/apple/Arut.xcodeproj` with `mise run surface:apple-project` when changing `project.yml`. Generated Xcode projects, derived data, Swift build products, and FFI packages are ignored by Git. All new labels originate in `product/i18n/locales/en/ui.ftl`; `mise run generate` regenerates the native resources and accessors for every platform.

## Observation

The observation idiom lives in `ArutBindings`, not in this surface: `Projection`, `Rows`, `Draft`, `observing` and `following` are the only places a generated `AsyncStream` is consumed (ADR 0021). `SessionState` and `ConversationState` are `@Observable @MainActor` classes that hold those helpers and wire them to the handles; a `.task { await state.run() }` starts one subscription per scope, because a scope revises its whole projection at once, and SwiftUI cancels that task when the view disappears, which releases the Rust subscriptions. `Rows` fetches every accepted message after the last key, so coalesced revisions drop no transcript entries. Observation tracks only the properties a `body` reads, so a sidebar does not re-render for a draft edit.

Rust owns the search predicate, the window title, the unread mark and end-of-speaker-group spacing. The sidebar writes `setQuery` and renders `state()`, the window title is `title()` with the new-conversation label as its only local fallback, and the transcript spaces after a row from `endsSpeakerGroup` instead of looking at the next one. `ChatHandle` and `ComposerHandle` conform to `ArutBindings.ErrorSource` in one file of retroactive conformances, and `localized(bundle:)` resolves the Fluent id against the String Catalog.

## Interaction conventions

- The sidebar uses native selection, search, resizing, and collapse behavior. Cmd-N opens the session's pending conversation. A pending draft remains shared until its first message establishes a conversation. The new-conversation action sits in the toolbar and in the sidebar's bottom bar, not as a row inside the selectable list, and the empty and no-results states fill the sidebar rather than occupying a list row.
- The plain text composer wraps and grows from one to seven visible lines, then scrolls within the field. Return sends and Option-Return inserts a line break, both through the native multiline field; the surface installs no key handler and never reaches into AppKit for the field editor. Cmd-Return also sends. Cmd-L focuses the composer. Cmd-F focuses search on macOS 15 and later. Control-Tab and Control-Shift-Tab navigate conversations. Cmd-, opens Settings and Shift-Cmd-/ opens the Help menu's shortcut reference.
- Keystrokes update the visible field synchronously and reach Rust as they are typed. Rust echoes the submitted text into the composer projection and coalesces rapid edits behind one in-flight write, last one winning, and a send flushes before it commits. The surface keeps no edit queue.
- Messages use selectable text, directional bubbles, grouped timestamps, and the native text-selection and copy context menu. Dates use the system locale. The app displays only accepted messages and actual sending state.
- The transcript, composer, and latest-message control share a centered column capped at 880 points. Narrow windows use the available width, while ultrawide windows keep both sides of the conversation within reading distance.
- The transcript fetches messages after its last key. New rows use a brief opacity transition; history does not replay an entrance animation when switching conversations. Reading positions are retained per conversation. New activity only follows the bottom when the reader is already near it; sending explicitly returns to the latest message.
- The composer keeps a padded, rounded Liquid Glass container with the send button inside. A native multiline text field and circular system button share the last text baseline; SwiftUI controls their sizing. The placeholder and entered text use the same field and insets. Earlier systems use a material background. The system bottom bar manages transcript separation. Message bubbles remain content. SwiftUI supplies the glass lighting and refraction, toolbar treatment, sidebar behavior, and scroll-edge separation. The app does not draw its own input border, position the placeholder, add a duplicate toolbar title, or overlay a custom scroll-edge fade.
- Reduce Motion disables the transcript transition. An incoming message posts an `AccessibilityNotification.Announcement`, so VoiceOver hears a reply the way it does on the other surfaces. Native controls handle Reduce Transparency and their own interaction feedback. Colors follow system appearance: the outgoing bubble takes the user's accent color rather than a literal blue, and incoming bubbles gain contrast when Increase Contrast is enabled. The bubble's maximum reading width is a `@ScaledMetric`, so it grows with Dynamic Type.
- The menu bar carries File, Edit, Conversations, Window and Help, plus the Settings item macOS adds for the `Settings` scene. Settings shows what the node's capability manifest reports; Help opens a window listing the keyboard shortcuts. The window remembers its selected conversation through `@SceneStorage` and resizes within the content's own minimums.

The reference is Messages on macOS Tahoe, alongside Apple's [Messages keyboard shortcuts](https://support.apple.com/en-euro/guide/messages/ichtc78b3bff/mac), [Meet Liquid Glass](https://developer.apple.com/videos/play/wwdc2025/219/), [new design system guidance](https://developer.apple.com/videos/play/wwdc2025/356/), and [SwiftUI implementation guidance](https://developer.apple.com/videos/play/wwdc2025/323/). The brief transcript transition is an implementation choice, not a measurement of Messages.

## Validation

This surface has not been compiled since the `ArutBindings` adoption: no Swift toolchain is available in the change's environment, so the observation, settings, help and accessibility work below is reviewed but unbuilt.

On macOS 26.6.2 with Xcode 26.6, the macOS app builds and launches, the Swift integration tests pass, and the architecture and localization checks pass. The full `mise run check` gate passes, including 115 Rust tests, and `mise run build` passes.

`ArutMacUITests` covers keyboard sending, multiline input beyond seven lines, and conversation draft restoration. Run it from the ArutMac scheme in Xcode with a signing setup allowed by the machine's security policy. On the development machine used for this change, macOS rejects both the unsigned and ad hoc signed XCTest UI runner before test execution. The UI target can be compiled with `build-for-testing`; its automated runtime result is not verified here. App interactions are checked separately through macOS accessibility automation. Direct checks verified Option-Return, a 12-line draft capped at seven visible lines, Cmd-Return sending with exact line breaks, and responsive layout at 740, 1,200, and 1,800 point window widths.

The installed iOS 26.4 simulators do not satisfy Xcode's missing iOS 26.5 platform component. Simulator execution remains unverified until that component is installed.
