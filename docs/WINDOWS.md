# Windows app

The desktop app uses WinUI 3 and C# over the Rust product session. Windows owns layout, keyboard input, accessibility, and window lifecycle. Rust owns conversations, drafts, and typed outcomes. The current FFI factory uses memory storage and an echo response. Closing the app loses the session; this app does not yet connect to the persistent native daemon.

## Build and run

Install mise and Visual Studio Build Tools with the Desktop development with C++ workload and Windows SDK. Run from the repository root in PowerShell:

```powershell
mise install
mise run surface:windows
```

The repository pins cargo-binstall and requires prebuilt Cargo tools. BoltFFI comes directly from its GitHub release through mise because binstall cannot discover its archive names. Mise fails if a binary is unavailable instead of compiling a tool from source. The app's Rust core still compiles locally. The binding task discovers MSVC through vswhere, so a Developer PowerShell terminal is optional.

The app references `bindings/dotnet/Arut.Bindings.csproj`, which holds the Rust-to-.NET idiom and brings the generated project with it. Debug and Release bindings live in separate `bindings/generated/csharp/Debug` and `Release` directories. MSBuild selects the directory through `$(Configuration)`, so publishing cannot replace the Debug native library. Generate the corresponding package through mise before opening a fresh checkout in an IDE. See [build workflows](BUILDING.md) for the shared tool setup.

The app targets Windows x64, .NET 10, and Windows App SDK 2.4. It runs unpackaged with the .NET and Windows App SDK runtimes beside the executable. `mise run publish:windows` builds Rust and C# in Release mode and writes the distributable folder to `surfaces/windows/bin/publish`. Distribute the entire folder, including `arut_ffi.dll`, `Arut.Windows.pri`, and the compiled `.xbf` views.

```powershell
mise run check:windows
```

This builds the generated bindings and WinUI app and checks formatting. It also checks generated string resources, `x:Uid` references, and shared architectural boundaries. The observation behaviour it used to test now lives in Rust and is covered by the workspace tests. Close the running app before rebuilding so Windows can replace its DLLs.

## Formatting

Run `mise run fmt:dotnet` to format the handwritten C#; add `--verify-no-changes` to check instead, which is what Windows CI runs. It uses `dotnet format` from the mise-pinned .NET SDK, so there is no tool manifest to restore.

The root `.editorconfig` sets four-space C# indentation, two-space XML indentation, and LF line endings, and `dotnet format` reads it.

## Interaction and state

The transcript fills the conversation panel behind a floating composer. Its surrounding space is transparent; the input card uses native in-app Acrylic over the existing Mica window. The floating composer, jump-to-latest control, and transient overlay sidebar use WinUI's in-app Acrylic brush; menus retain their native material. There is no custom blur renderer. A scrollable footer follows the composer's measured height so the last message and timestamp can clear the input, while the scrollbar retains the full viewport height. The jump-to-latest button sits above the composer. Item insertion, conversation switching, and composer resizing do not animate layout.

The composer and jump-to-latest card use [WinUI ThemeShadow](https://learn.microsoft.com/en-us/windows/apps/develop/ui/shadows); the button retains its native interaction states over an Acrylic backing. The composer uses the native focused text-control border brush while keeping its Acrylic fill and fixed native shadow. Its inner TextBox system-focus rectangle is disabled; the whole card indicates focus for both keyboard and pointer input. Buttons retain their native keyboard focus visuals.

The executable, taskbar, and native titlebar use the shared Arut icon. See [app icon assets](../product/brand/README.md) for the master artwork, platform formats, and regeneration command.

The conversation list derives from WinUI's `DefaultListViewItemStyle`, retaining its rounded selection background, accent indicator, hover states, and keyboard focus treatment. Toolbar buttons use `ControlCornerRadius`; the conversation panel and icon containers use `OverlayCornerRadius`. Caption typography and spacing follow the shared Fluent resources. The composer uses a 27-DIP radius to form a pill at its normal 54-DIP height and retains those corners when multiline. Message bubbles and circular send/scroll buttons retain their chat-specific shapes.

The compact 32-DIP WinUI TitleBar contains the current conversation title, history toggle, search, and new-conversation actions while retaining Windows snap layouts and system controls. The title uses the flexible content column so long names cannot push actions under the caption buttons. Below 400 logical pixels, the optional conversation title hides to preserve space for the app identity and actions. Mica and theme resources follow Windows appearance. History changes to a light-dismiss Acrylic overlay below 800 logical pixels, using the native solid fallback when transparency is unavailable. The manifest declares Per-Monitor V2 DPI awareness to prevent Windows from bitmap-scaling the window.

Virtualized transcript rows distinguish outgoing and incoming messages with alignment, color, and grouped spacing. Each bubble shows local time, with a full timestamp tooltip and separators after five-minute gaps. Text supports native selection and a copy context menu. The transcript follows the tail unless the reader has scrolled away; `ItemsStackPanel.ItemsUpdatingScrollMode` does the anchoring. A jump-to-latest button appears when reading older messages.

Rust's chat projection supplies `starts_time_group`, `starts_speaker_group` and `ends_speaker_group` with each keyed message range, so a row's trailing space comes from the row itself rather than from the one behind it. Windows, Apple, and Linux use the same grouping policy; each retains its own date formatting and layout. The first row in a range is compared with its stored predecessor, so incremental reads preserve group boundaries without another FFI call per message.

The native TextBox sizes wrapped text at its available width, grows up to 164 DIPs, and shrinks when text is removed or sent. The transcript fills the available space and retains ListView virtualization, including when the conversation is short.

Message rows are two `DataTemplate`s over an immutable `MessageRow` record, selected by `IsOutgoing`; there is no per-row control, no dependency property, and no send transition. Bubbles use `{ThemeResource}` brushes aliased in the control's own theme dictionaries, so light and dark take the Fluent layer and accent brushes and High Contrast takes the system window and highlight colors. Transcript rows are focusable and single-selectable; Ctrl+C copies the focused row and each bubble carries its own copy flyout with the message as the command parameter, so copying is not mouse-only.

The conversation list is reconciled in place against what Rust publishes: rows that stayed keep their containers, so the pane keeps its scroll offset and the selection never flickers through -1. `SelectedIndex` two-way is the only path into the selection. Conversation view models are bounded: the eight most recently shown keep their handles and pumps, and the rest are disposed. Rust keeps every draft, so an evicted conversation loses nothing but its warm start.

## Reference implementations

The September 2026 review inspected source at pinned commits rather than copying dependency lists or entire custom controls:

| Reference | Applied finding |
| --- | --- |
| [PowerToys Hosts](https://github.com/microsoft/PowerToys/blob/8e832ee72dcffb5df297ceb9e06a04797375092c/src/modules/Hosts/Hosts/HostsXAML/MainWindow.xaml.cs) | Retain native TitleBar, caption controls, and explicit window lifetime. Its larger window framework is unnecessary here. |
| [WinUI Gallery collection animation](https://github.com/microsoft/WinUI-Gallery/blob/be5624441fa8667761053bb5bae4db5d5a7ad4e7/WinUIGallery/SampleSupport/SamplePages/CollectionPage.xaml.cs) | Realize and lay out the destination before starting. |
| [WinUI connected-animation implementation](https://github.com/microsoft/microsoft-ui-xaml/blob/f881b81bfbc89c5cc7cfe2a3d377cdd153406656/dxaml/xcp/components/animation/ConnectedAnimation.cpp) | Its unscaled snapshot path and centering behavior informed the replacement with a composition-based layout helper. |
| [Rebound resource ownership](https://github.com/IviriusCommunity/Rebound/blob/b746ea9ec9689049b8759fdb07fedd7a3c130c23/src/core/Rebound.Core.UI/UserControls/HomePageHeaderImage.xaml.cs) | Give animation resources an explicit view lifetime. Avoid importing its custom caption/nonclient stack. |
| [Atlas Toolbox window](https://github.com/Atlas-OS/atlas-toolbox/blob/64586818dcee7da5d7773a635176932ea7be1a17/AtlasToolbox/MainWindow.xaml) | Use native controls, font icons, and visual-state resources. Fixed titlebar search widths do not suit this app's compact layout. |

Both community apps use CommunityToolkit animation helpers for ordinary fades and transforms. The stable Animations package's [connected-animation helper](https://github.com/CommunityToolkit/Windows/blob/413892f3e929beae3fbcf9863b8385e570407a41/components/Animations/src/ConnectedAnimations/ConnectedAnimationHelper.cs) wraps WinUI's API for Frame navigation. This app now ships no send transition, so it takes neither helper and no prerelease feed. CommunityToolkit.Mvvm supplies the `INotifyPropertyChanged` and `ICommand` plumbing; Rust still owns ordering and acknowledgement.

| Shortcut | Action |
| --- | --- |
| Ctrl+N | New conversation |
| Ctrl+B | Toggle history |
| Ctrl+L | Focus composer |
| Ctrl+F | Search conversations |
| Ctrl+C | Copy the focused transcript row |
| Enter or Ctrl+Enter | Send |
| Shift+Enter or Alt+Enter | Insert a line break |

`ConversationViewModel` derives from `ObservableObject`; `[ObservableProperty]` partial properties and `[RelayCommand]` generate the notification and command plumbing. Each resident conversation keeps its own view model, so switching cannot redirect a pending draft command to another conversation. `ChatViewModel` owns which conversation is selected and which view models are alive; Rust owns the list, the search that narrows it, the unread mark and the selected conversation's title.

The observation idiom lives in `Arut.Bindings`, not in this surface (ADR 0021). `Projection<T>` runs the one `await foreach` per generated `IAsyncEnumerable<ulong>` stream and re-reads the projection that revision names; `Rows<T>` appends by the last accepted message ID; `Draft` echoes locally and then writes, and Rust coalesces the rest; `Following.RunAsync` owns the composer's initialize-then-follow lifetime; `Reconcile.Apply` brings the conversation list to what Rust publishes; `ErrorText.Describe` turns a Fluent id and its `ErrorArg` list into a sentence through the app's PRI resources; `Time.AcceptedAt` bounds an epoch-millisecond stamp. The pumps start on the UI thread, so their continuations resume there without an injected dispatcher. Closing cancels the pumps, waits for them, then disposes the native handles before closing the window.

The project follows Microsoft's [stable release guidance](https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/release-channels) and [unpackaged deployment model](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/unpackage-winui-app). Tool installation follows [mise's Cargo backend](https://mise.jdx.dev/dev-tools/backends/cargo).

## Validation

This surface has not been compiled since the `Arut.Bindings` adoption: no .NET SDK or Windows toolchain is available in the change's environment, so the view-model, theming, selection and keyboard work above is reviewed but unbuilt.

The Windows Release build passes with zero warnings. UI Automation verified three composer grow/clear cycles, independent drafts, and sending the final value after 30 consecutive edits. Keyboard input verified that Shift+Enter inserts a newline and Enter sends the exact two-line message. Twenty consecutive sends verified following the latest message and the jump-to-latest action after scrolling up.

Screenshots at 1100, 560, and 420 physical pixels checked wide, collapsed, and overlay layouts at 125% scaling. The compact caption and actions do not overlap. `DesktopTitleBar` corrects the pinned WinUI control's physical-pixel caption insets to logical units. Window closure completed after canceling native work. Multi-monitor DPI transitions, IME composition, and reduced-motion appearance still need interactive review on appropriate hardware/settings.

The floating-composer checks verified that the transcript extends behind the input and that the final message clears it after draft growth, clearing, and a narrow resize. Screenshots also checked the composer and jump-to-latest button over the middle of a scrolling transcript.

