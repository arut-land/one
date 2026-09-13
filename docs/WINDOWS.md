# Windows app

The desktop app uses WinUI 3 and C# over the Rust product session. Windows owns layout, keyboard input, accessibility, and window lifecycle. Rust owns conversations, drafts, and typed outcomes. The current FFI factory uses memory storage and an echo response. Closing the app loses the session; this app does not yet connect to the persistent native daemon.

## Build and run

Install mise and Visual Studio Build Tools with the Desktop development with C++ workload and Windows SDK. Run from the repository root in PowerShell:

```powershell
mise install
mise run surface:windows
```

The repository pins cargo-binstall and requires prebuilt Cargo tools. BoltFFI comes directly from its GitHub release through mise because binstall cannot discover its archive names. Mise fails if a binary is unavailable instead of compiling a tool from source. The app's Rust core still compiles locally. The binding task discovers MSVC through vswhere, so a Developer PowerShell terminal is optional.

Debug and Release bindings live in separate `bindings/generated/csharp/Debug` and `Release` directories. MSBuild selects the directory through `$(Configuration)`, so publishing cannot replace the Debug native library. Generate the corresponding package through mise before opening a fresh checkout in an IDE. See [build workflows](BUILDING.md) for the shared tool setup.

The app targets Windows x64, .NET 10, and Windows App SDK 2.4. It runs unpackaged with the .NET and Windows App SDK runtimes beside the executable. `mise run publish:windows` builds Rust and C# in Release mode and writes the distributable folder to `surfaces/windows/bin/publish`. Distribute the entire folder, including `arut_ffi.dll`, `Arut.Windows.pri`, and the compiled `.xbf` views.

```powershell
mise run check:windows
```

This builds the generated bindings and WinUI app, checks formatting, and runs the .NET observation tests. `mise run check:observers-dotnet` runs those tests without WinUI or a native library. Close the running app before rebuilding so Windows can replace its DLLs.

## Formatting

Run `mise run fmt:dotnet` to format handwritten C#, XAML, project XML, and the app manifest. `mise run check:format-dotnet` checks the same files without modifying them and runs in Windows CI. Both restore CSharpier 1.3.0 from `dotnet-tools.json` using the mise-pinned .NET SDK.

The root `.editorconfig` sets four-space C# indentation, two-space XML indentation, and LF line endings. CSharpier preserves significant XML text whitespace. `.csharpierignore` excludes generated exports, bindings, localized resources, and build outputs. Install the [CSharpier editor extension](https://csharpier.com/docs/Editors) for format-on-save with the repository's pinned version. Its [configuration documentation](https://csharpier.com/docs/Configuration) covers the shared EditorConfig settings.

## Interaction and state

The transcript fills the conversation panel behind a floating composer. Its surrounding space is transparent; the input card uses native in-app Acrylic over the existing Mica window. The floating composer, jump-to-latest control, and transient overlay sidebar use WinUI's in-app Acrylic brush; menus retain their native material. There is no custom blur renderer. A scrollable footer follows the composer's measured height so the last message and timestamp can clear the input, while the scrollbar retains the full viewport height. The jump-to-latest button sits above the composer. Item insertion, conversation switching, and composer resizing do not animate layout. Early replies remain in Rust until the outgoing morph completes, then enter the transcript with a native fade, without reserving blank space first.

Native gradient fades indicate hidden messages at the top and beneath the composer. They disappear at the corresponding content boundary and never intercept input. Their transparent stops retain the theme color's RGB values to avoid a pale band during interpolation. The composer and jump-to-latest card use [WinUI ThemeShadow](https://learn.microsoft.com/en-us/windows/apps/develop/ui/shadows); the button retains its native interaction states over an Acrylic backing. The composer uses the native focused text-control border brush while keeping its Acrylic fill and fixed native shadow. Its inner TextBox system-focus rectangle is disabled; the whole card indicates focus for both keyboard and pointer input. Buttons retain their native keyboard focus visuals.

The executable, taskbar, and native titlebar use the shared Arut icon. See [app icon assets](../product/brand/README.md) for the master artwork, platform formats, and regeneration command.

The conversation list derives from WinUI's `DefaultListViewItemStyle`, retaining its rounded selection background, accent indicator, hover states, and keyboard focus treatment. Toolbar buttons use `ControlCornerRadius`; the conversation panel and icon containers use `OverlayCornerRadius`. Caption typography and spacing follow the shared Fluent resources. The composer uses a 27-DIP radius to form a pill at its normal 54-DIP height and retains those corners when multiline. Message bubbles and circular send/scroll buttons retain their chat-specific shapes.

The compact 32-DIP WinUI TitleBar contains the current conversation title, history toggle, search, and new-conversation actions while retaining Windows snap layouts and system controls. The title uses the flexible content column so long names cannot push actions under the caption buttons. Below 400 logical pixels, the optional conversation title hides to preserve space for the app identity and actions. Mica and theme resources follow Windows appearance. History changes to a light-dismiss Acrylic overlay below 800 logical pixels, using the native solid fallback when transparency is unavailable. The manifest declares Per-Monitor V2 DPI awareness to prevent Windows from bitmap-scaling the window.

Virtualized transcript rows distinguish outgoing and incoming messages with alignment, color, and grouped spacing. Each bubble shows local time, with a full timestamp tooltip and separators after five-minute gaps. Text supports native selection and a copy context menu. Each conversation remembers its first visible message and that row's position in the viewport, rather than a global offset based on estimated row heights. A jump-to-latest button appears when reading older messages.

Rust's chat projection supplies `starts_time_group` and `starts_speaker_group` with each keyed message range. Windows, Apple, and Linux use the same grouping policy; each retains its own date formatting and layout. The first row in a range is compared with its stored predecessor, so incremental reads preserve group boundaries without another FFI call per message.

The native TextBox sizes wrapped text at its available width, grows up to 164 DIPs, and shrinks when text is removed or sent. A submitted message gets a separate, fixed source layout for its transition, so the live editor can clear, resize, and accept new input immediately. The transcript fills the available space and retains ListView virtualization, including when the conversation is short.

The chat view owns one disposable send transition using Community Toolkit Labs `TransitionHelper`. Sibling background and text elements share IDs between the frozen source and the outgoing bubble. The background uses `ScaleMode.Scale`; text uses `ScaleMode.None` with a top-left origin. The helper computes geometry, composition animations, and crossfades. New timestamp content fades in with no translation. Message controls contain declarative IDs and render data; they do not queue shared-element callbacks or manage global visibility.

Replies accepted during a send stay in Rust until the outgoing transition completes. They are not inserted as hidden rows, so no reply space is reserved during the morph. Completion releases them into the visible list in order with a native `FadeInThemeAnimation`. Later replies use the same entrance. Cancellation releases accepted replies immediately without delaying transport or leaving messages hidden. A scroll request raised during reply release is retained for the next layout pass. Layout changes themselves are not animated.

The view associates each transition with its exact accepted outgoing row and realizes the destination before starting. A new send, navigation, geometry change, scrolling, target recycling, expiry, or disposal cancels the previous transition and restores the target. Clipped destinations and internally scrolling drafts skip the morph. Motion respects Windows' animation setting. Conversation switching and composer resizing have no custom layout animation.

The helper animates live visuals rather than native connected-animation snapshots. A frozen source avoids mutating submitted content while it moves. Text stays unscaled; different line wraps still crossfade rather than continuously reflowing. The duration is 220ms, with explicit per-element crossfade settings and a delayed timestamp fade. There are no hand-calculated flight paths.

`CommunityToolkit.Labs.WinUI.TransitionHelper` is pinned to `0.1.260617-build.2640`. It is experimental and depends on Toolkit Animations and Behaviors `8.3.260402-preview2`. `NuGet.Config` maps only `CommunityToolkit.Labs.*` to Microsoft's Toolkit Labs feed; other packages use NuGet.org. The package compiled and restored with this app's pinned .NET and Windows App SDK. Keep the package pin and run the interaction checks when upgrading it.

## Reference implementations

The September 2026 review inspected source at pinned commits rather than copying dependency lists or entire custom controls:

| Reference | Applied finding |
| --- | --- |
| [PowerToys Hosts](https://github.com/microsoft/PowerToys/blob/8e832ee72dcffb5df297ceb9e06a04797375092c/src/modules/Hosts/Hosts/HostsXAML/MainWindow.xaml.cs) | Retain native TitleBar, caption controls, and explicit window lifetime. Its larger window framework is unnecessary here. |
| [WinUI Gallery collection animation](https://github.com/microsoft/WinUI-Gallery/blob/be5624441fa8667761053bb5bae4db5d5a7ad4e7/WinUIGallery/SampleSupport/SamplePages/CollectionPage.xaml.cs) | Realize and lay out the destination before starting. |
| [WinUI connected-animation implementation](https://github.com/microsoft/microsoft-ui-xaml/blob/f881b81bfbc89c5cc7cfe2a3d377cdd153406656/dxaml/xcp/components/animation/ConnectedAnimation.cpp) | Its unscaled snapshot path and centering behavior informed the replacement with a composition-based layout helper. |
| [Rebound resource ownership](https://github.com/IviriusCommunity/Rebound/blob/b746ea9ec9689049b8759fdb07fedd7a3c130c23/src/core/Rebound.Core.UI/UserControls/HomePageHeaderImage.xaml.cs) | Give animation resources an explicit view lifetime. Avoid importing its custom caption/nonclient stack. |
| [Atlas Toolbox window](https://github.com/Atlas-OS/atlas-toolbox/blob/64586818dcee7da5d7773a635176932ea7be1a17/AtlasToolbox/MainWindow.xaml) | Use native controls, font icons, and visual-state resources. Fixed titlebar search widths do not suit this app's compact layout. |

Both community apps use CommunityToolkit animation helpers for ordinary fades and transforms. The stable Animations package's [connected-animation helper](https://github.com/CommunityToolkit/Windows/blob/413892f3e929beae3fbcf9863b8385e570407a41/components/Animations/src/ConnectedAnimations/ConnectedAnimationHelper.cs) wraps WinUI's API for Frame navigation. The separate [Labs TransitionHelper](https://github.com/CommunityToolkit/Labs-Windows/blob/13a9bf4ebfec272bfe2ba9875b21a5e09829ad74/components/TransitionHelper/src/TransitionHelper.Logic.cs) supplies the ID matching, per-element scaling, crossfades, and cancellation needed here. CommunityToolkit.Mvvm would not replace Rust's ordered mutations or observer acknowledgement rules.

| Shortcut | Action |
| --- | --- |
| Ctrl+N | New conversation |
| Ctrl+B | Toggle history |
| Ctrl+L | Focus composer |
| Ctrl+F | Search conversations |
| Enter or Ctrl+Enter | Send |
| Shift+Enter or Alt+Enter | Insert a line break |

`ConversationModel` holds presentation properties through `INotifyPropertyChanged`. Each visited conversation retains its model so switching cannot redirect pending draft commands to another conversation. Hidden conversations stop their revision subscriptions and reread state when selected. Draft edits and sends share an ordered task queue. Only fresh composer snapshots acknowledge edits; transcript updates never write a cached draft back into the input. Local typing does not echo a `Draft` property notification back into the TextBox.

The shared `ObservableState<T>` adapter coalesces invalidations before posting to WinUI's DispatcherQueue. Explicit dispatcher injection avoids relying on an ambient synchronization context. Transcript reads use the last accepted message ID and append only new rows. Closing cancels observations and asynchronous work, waits for completion, then disposes the native handles before closing the window.

The project follows Microsoft's [stable release guidance](https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/release-channels) and [unpackaged deployment model](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/unpackage-winui-app). Tool installation follows [mise's Cargo backend](https://mise.jdx.dev/dev-tools/backends/cargo).

## Validation

The Windows Release build passes with zero warnings, and all eight .NET observer tests pass. UI Automation verified three composer grow/clear cycles, independent drafts, and sending the final value after 30 consecutive edits. Keyboard input verified that Shift+Enter inserts a newline and Enter sends the exact two-line message. Twenty consecutive sends verified following the latest message and the jump-to-latest action after scrolling up.

Screenshots at 1100, 560, and 420 physical pixels checked wide, collapsed, and overlay layouts at 125% scaling. The compact caption and actions do not overlap. `DesktopTitleBar` corrects the pinned WinUI control's physical-pixel caption insets to logical units. Window closure completed after canceling native work. Multi-monitor DPI transitions, IME composition, and reduced-motion appearance still need interactive review on appropriate hardware/settings.

The floating-composer checks verified that the transcript extends behind the input and that the final message clears it after draft growth, clearing, and a narrow resize. Screenshots also checked the composer and jump-to-latest button over the middle of a scrolling transcript.

See the [Windows implementation audit](WINDOWS-AUDIT.md) for platform decisions, retained workarounds, and verification limits.
