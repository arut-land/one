# Apple app

The Apple app uses SwiftUI over `ArutBindings`. The composition root creates the product session; views observe the Rust projections and invoke their commands. The current FFI factory uses memory ports. Conversations survive switching within the session, but restarting the app does not persist them. The native daemon's redb store is not connected to this app yet.

## Build and run

Use Xcode 26 or newer to compile the Liquid Glass APIs. The app still targets macOS 13 and iOS 16. Native Liquid Glass controls and `safeAreaBar` are enabled on macOS/iOS 26; earlier systems use bordered controls and `safeAreaInset`. Swift integration tests use the Swift Testing framework bundled with Xcode. The optional macOS UI-test target requires macOS 14 or later.

```sh
mise run check:apple
open surfaces/apple/DerivedData/Build/Products/Debug/ArutMac.app
```

`check:apple` installs the Apple Rust targets, packages the XCFramework, generates the Xcode project, builds ArutMac without distribution signing, and runs the Swift binding tests. CI runs this task on pull requests and release tags.

For an incremental Swift-only build, after generating the framework and project:

```sh
mise run --skip-deps surface:macos
```

Regenerate `surfaces/apple/Arut.xcodeproj` with `mise run surface:apple-project` when changing `project.yml`. Generated Xcode projects, derived data, Swift build products, and FFI packages are ignored by Git. All new labels originate in `product/i18n/locales/en/ui.ftl`; `mise run i18n` regenerates the native resources and accessors for every platform.

## Interaction conventions

- The sidebar uses native selection, search, resizing, and collapse behavior. Cmd-N opens the session's pending conversation. A pending draft remains shared until its first message establishes a conversation.
- The plain text composer wraps and grows from one to seven visible lines, then scrolls within the field. Return sends; Option-Return inserts a line break through the native multiline field. On macOS 14 and later, Shift-Return invokes the native field editor’s newline command, preserving selection and undo; marked text is left to the input method. Cmd-Return also sends. Cmd-L focuses the composer. Cmd-F focuses search on macOS 15 and later. Control-Tab and Control-Shift-Tab navigate conversations.
- Keystrokes update the visible field synchronously. Each replacement still reaches Rust in order, and sending waits for outstanding replacements. This preserves the existing per-edit acknowledgement contract.
- Messages use selectable text, directional bubbles, grouped timestamps, and the native text-selection and copy context menu. Dates use the system locale. The app displays only accepted messages and actual sending state.
- The transcript, composer, and latest-message control share a centered column capped at 880 points. Narrow windows use the available width, while ultrawide windows keep both sides of the conversation within reading distance.
- The transcript fetches messages after its last key. New rows use a brief opacity transition; history does not replay an entrance animation when switching conversations. Reading positions are retained per conversation on macOS 14/iOS 17 and later. New activity only follows the bottom when the reader is already near it; sending explicitly returns to the latest message.
- The composer keeps a padded, rounded Liquid Glass container with the send button inside. A native multiline text field and circular system button share the last text baseline; SwiftUI controls their sizing. The placeholder and entered text use the same field and insets. Earlier systems use a material background. The system bottom bar manages transcript separation. Message bubbles remain content. SwiftUI supplies the glass lighting and refraction, toolbar treatment, sidebar behavior, and scroll-edge separation. The app does not draw its own input border, position the placeholder, add a duplicate toolbar title, or overlay a custom scroll-edge fade.
- Reduce Motion disables the transcript transition. Native controls handle Reduce Transparency and their own interaction feedback. Colors follow system appearance; incoming bubbles gain contrast when Increase Contrast is enabled.

The reference is Messages on macOS Tahoe, alongside Apple's [Messages keyboard shortcuts](https://support.apple.com/en-euro/guide/messages/ichtc78b3bff/mac), [Meet Liquid Glass](https://developer.apple.com/videos/play/wwdc2025/219/), [new design system guidance](https://developer.apple.com/videos/play/wwdc2025/356/), and [SwiftUI implementation guidance](https://developer.apple.com/videos/play/wwdc2025/323/). The brief transcript transition is an implementation choice, not a measurement of Messages.

## Validation

On macOS 26.6.2 with Xcode 26.6, the macOS app builds and launches, the Swift integration tests pass, and the binding export, architecture, and localization checks pass. The full `mise run check` gate passes, including 116 Rust tests, and `mise run build` passes.

`ArutMacUITests` covers keyboard sending, multiline input beyond seven lines, and conversation draft restoration. Run it from the ArutMac scheme in Xcode with a signing setup allowed by the machine's security policy. On the development machine used for this change, macOS rejects both the unsigned and ad hoc signed XCTest UI runner before test execution. The UI target can be compiled with `build-for-testing`; its automated runtime result is not verified here. App interactions are checked separately through macOS accessibility automation. Direct checks verified Shift-Return, Option-Return, a 12-line draft capped at seven visible lines, Cmd-Return sending with exact line breaks, and responsive layout at 740, 1,200, and 1,800 point window widths.

The installed iOS 26.4 simulators do not satisfy Xcode's missing iOS 26.5 platform component. Simulator execution remains unverified until that component is installed.
