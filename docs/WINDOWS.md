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

The app targets Windows x64, .NET 10 (`net10.0-windows10.0.26100.0`), Windows App SDK 2.4.0 and CommunityToolkit.Mvvm 8.4.2, each the current stable release. It runs unpackaged with the .NET and Windows App SDK runtimes beside the executable. `mise run publish:windows` builds Rust and C# in Release mode and writes the distributable folder to `surfaces/windows/bin/publish`. Distribute the entire folder, including `arut_ffi.dll`, `Arut.Windows.pri`, and the compiled `.xbf` views.

```powershell
mise run check:windows
```

This regenerates localization and handle descriptors, builds the generated bindings and WinUI app, and checks formatting. `mise exec -- cargo run --quiet -p arut-dev -- check` checks generated string resources, `x:Uid` references, and shared architectural boundaries. The observation behaviour it used to test now lives in Rust and is covered by the workspace tests. Close the running app before rebuilding so Windows can replace its DLLs.

## Formatting

Run `mise run fmt:dotnet` to format the handwritten C#; add `--verify-no-changes` to check instead, which is what Windows CI runs. It uses `dotnet format` from the mise-pinned .NET SDK, so there is no tool manifest to restore.

The root `.editorconfig` sets four-space C# indentation, two-space XML indentation, and LF line endings, and `dotnet format` reads it.

## Interaction and state

The transcript fills the conversation panel behind a floating composer. Its surrounding space is transparent; the input card uses native in-app Acrylic over the existing Mica window. The floating composer, jump-to-latest control, and transient overlay sidebar use WinUI's in-app Acrylic brush; menus retain their native material. There is no custom blur renderer. A scrollable footer follows the composer's measured height so the last message and timestamp can clear the input, while the scrollbar retains the full viewport height. The jump-to-latest button sits above the composer. Item insertion, conversation switching, and composer resizing do not animate layout.

The composer and jump-to-latest control use [WinUI ThemeShadow](https://learn.microsoft.com/en-us/windows/apps/develop/ui/shadows). The latest control is one native Button, elevated by its layout container so the button's visual can handle interaction and visibility animation without displacing its hit area. Its hover and pressed resources retain Acrylic; High Contrast retains system highlight fills. The composer container owns its single border, focus indication and material. Local TextBox theme resources remove the editor's nested fill and focused border without replacing its template. TextBox still owns editing, selection, undo, IME and scrolling. Buttons retain their native keyboard focus visuals.

The executable, taskbar, and native titlebar use the shared Arut icon. See [app icon assets](../product/brand/README.md) for the master artwork, platform formats, and regeneration command.

The conversation list derives from WinUI's `DefaultListViewItemStyle`, retaining its rounded selection background, accent indicator, hover states, and keyboard focus treatment. Toolbar buttons use `ControlCornerRadius`; the conversation panel and icon containers use `OverlayCornerRadius`. Caption typography and spacing follow the shared Fluent resources. The composer uses a 27-DIP radius to form a pill at its normal 54-DIP height and retains those corners when multiline. Message bubbles and circular send/scroll buttons retain their chat-specific shapes.

The compact 32-DIP WinUI TitleBar contains the current conversation title, history toggle, search, and new-conversation actions while retaining Windows snap layouts and system controls. The title uses the flexible content column so long names cannot push actions under the caption buttons. Below 400 logical pixels, the optional conversation title hides to preserve space for the app identity and actions. Mica and theme resources follow Windows appearance. History changes to a light-dismiss Acrylic overlay below 800 logical pixels, using the native solid fallback when transparency is unavailable. The manifest declares Per-Monitor V2 DPI awareness to prevent Windows from bitmap-scaling the window.

Virtualized transcript rows distinguish outgoing and incoming messages with alignment, color, and grouped spacing. Each bubble shows local time, with a full timestamp tooltip and separators after five-minute gaps. Text supports native selection and a copy context menu. The transcript follows the tail unless the reader has scrolled away; `ItemsStackPanel.ItemsUpdatingScrollMode` does the anchoring. A jump-to-latest button appears when reading older messages.

Rust's chat projection supplies `starts_time_group`, `starts_speaker_group` and `ends_speaker_group` with each keyed message range, so a row's trailing space comes from the row itself rather than from the one behind it. Windows, Apple, and Linux use the same grouping policy; each retains its own date formatting and layout. The first row in a range is compared with its stored predecessor, so incremental reads preserve group boundaries without another FFI call per message.

The native TextBox sizes wrapped text at its available width, grows up to 164 DIPs, and shrinks when text is removed or sent. The transcript fills the available space and retains ListView virtualization, including when the conversation is short.

Message rows are two `DataTemplate`s over an immutable `MessageRow` record, selected by `IsOutgoing`. Bubbles use `{ThemeResource}` brushes aliased in the control's own theme dictionaries, so light and dark take the Fluent layer and accent brushes and High Contrast takes the system window and highlight colors. Transcript rows have selection disabled and transparent pointer-over/pressed backgrounds. Keyboard focus remains available. Ctrl+C preserves native text-range copying or copies the focused row when no text range is selected; each bubble also has a copy flyout.

The conversation list binds stable `ConversationRow` objects with Toolkit-generated observable properties. FFI summaries contain arrays whose default equality is by reference, so binding them directly would replace unchanged rows on every projection. Rows update their displayed fields in place; `Reconcile.Apply` handles actual insertion, removal and ordering. Switching the active chat does not reorder the list. Native `RepositionThemeTransition` handles structural changes only. Selection leaves keyboard focus in history; deliberate item activation transfers focus to the composer. Conversation view models are bounded to eight resident instances; Rust retains every draft.

Sending uses two native [connected animations](https://learn.microsoft.com/en-us/windows/apps/develop/motion/connected-animation), with a 167 ms basic configuration and an explicit fast-out, slow-in curve. Temporary native Border and TextBlock elements transfer the background and text while the live composer stays intact. The text moves without scaling or capturing the editor's caret. Its visual stays alive until completion or cancellation; conversation changes cancel before compiled bindings replace the old templates. Incoming replies remain in the model and layout, but their reveal and accessibility announcement wait for both outgoing animations to finish, then fade in over 83 ms. The outgoing timestamp remains at its final layout position, hidden during travel, and fades in over the same 83 ms phase. No network or core work waits for presentation. A new send, conversation switch, pane change, resize, recycled destination, or one-second expiry cancels the transition and releases any waiting reply. Scrolled multiline editors skip the transfer rather than animate a clipped fragment.

SplitView owns pane and content translation. A short fade softens the history content's entrance; compositor show/hide animations handle the empty state and jump-to-latest control. Custom motion follows `UISettings.AnimationsEnabled` and cancels when the setting changes. Native controls retain their own press, hover and focus behavior. `ActionButton` observes the native Button CommonStates to animate its content over 83 ms, without duplicating pointer or keyboard state. A content Grid keeps that transform separate from AnimatedIcon's internal transform. The latest action uses WinUI's animated down-chevron, appears beyond 120 DIPs from the tail, and remains visible until within 48 DIPs. Clicking uses ScrollViewer's native animated ChangeView; automatic tail-following stays immediate. Transition completion reevaluates the latest action so scrolling during a send cannot leave it hidden.

## Reference implementations

The Files comparison used commit `495c6e693555a1471a041a9cae2de3fbbed3ffe0`, cloned outside the repository into the Windows temporary directory. Its [toolbar control](https://github.com/files-community/Files/blob/495c6e693555a1471a041a9cae2de3fbbed3ffe0/src/Files.App.Controls/Toolbar/ToolbarButton/ToolbarButton.cs) extends native Button, with [separate theme resources](https://github.com/files-community/Files/blob/495c6e693555a1471a041a9cae2de3fbbed3ffe0/src/Files.App.Controls/Toolbar/ToolbarButton/ToolbarButton.ThemeResources.xaml). Arut follows that separation in `Themes/ChatResources.xaml`, retaining the platform Button template. ActionButton refreshes its feedback geometry after loading, sizing, content replacement, and template replacement. Search uses the native animated find icon; new-chat, search, send, and latest actions share the same native-state-driven feedback and reduced-motion setting.

Files' [window implementation](https://github.com/files-community/Files/blob/495c6e693555a1471a041a9cae2de3fbbed3ffe0/src/Files.App/Data/Items/WindowEx.cs) pairs GetWindowPlacement with SetWindowPlacement. Arut now does the same to remember normal bounds and maximized state in `%LOCALAPPDATA%/Arut/window.json`. Workspace coordinates stay within those Win32 APIs, including when the taskbar changes or a monitor disappears. A minimized window reopens normally or maximized as appropriate. Missing, unreadable, or malformed preferences leave the default window usable. This stores desktop placement only, not conversation data. The native window title also follows the selected conversation so Alt+Tab and taskbar previews identify it.

Full timestamp tooltips belong to timestamp text, rather than the entire message bubble. This avoids covering message text with time metadata during ordinary pointer movement. Connected-animation targets and their completion ordering remain unchanged.

The placement checks also exposed an existing minimize/restore crash in `DesktopTitleBar`: the runtime briefly reported `RightInset = -19` during restoration, and the DPI correction constructed an invalid negative GridLength. The correction now waits for nonnegative insets and a visible, non-minimized host. It retains the last valid padding during that transition. The failure reproduced in the preceding build and disappeared with the inset validation.

Files' [tab-drop indicator](https://github.com/files-community/Files/blob/495c6e693555a1471a041a9cae2de3fbbed3ffe0/src/Files.App/Views/ShellPanesPage.xaml.cs) uses Composition for feedback that its controls do not provide and respects the Windows animation preference. Arut already uses native connected animations for the send transfer and native SplitView for pane movement. Files pins the same Windows App SDK 2.4.0 and MVVM Toolkit 8.4.2 versions. Its filesystem services, shell replacement, tray lifetime, and packaged activation model do not map to this unpackaged chat application; they were not imported.

The September 2026 review inspected source at pinned commits rather than copying dependency lists or entire custom controls:

| Reference | Applied finding |
| --- | --- |
| [PowerToys Hosts](https://github.com/microsoft/PowerToys/blob/8e832ee72dcffb5df297ceb9e06a04797375092c/src/modules/Hosts/Hosts/HostsXAML/MainWindow.xaml.cs) | Retain native TitleBar, caption controls, and explicit window lifetime. Its larger window framework is unnecessary here. |
| [WinUI Gallery collection animation](https://github.com/microsoft/WinUI-Gallery/blob/be5624441fa8667761053bb5bae4db5d5a7ad4e7/WinUIGallery/SampleSupport/SamplePages/CollectionPage.xaml.cs) | Realize and lay out the destination before starting. |
| [WinUI connected-animation implementation](https://github.com/microsoft/microsoft-ui-xaml/blob/f881b81bfbc89c5cc7cfe2a3d377cdd153406656/dxaml/xcp/components/animation/ConnectedAnimation.cpp) | Use independent background and text snapshots so only the background scales. |
| [Rebound resource ownership](https://github.com/IviriusCommunity/Rebound/blob/b746ea9ec9689049b8759fdb07fedd7a3c130c23/src/core/Rebound.Core.UI/UserControls/HomePageHeaderImage.xaml.cs) | Give animation resources an explicit view lifetime. Avoid importing its custom caption/nonclient stack. |
| [Atlas Toolbox window](https://github.com/Atlas-OS/atlas-toolbox/blob/64586818dcee7da5d7773a635176932ea7be1a17/AtlasToolbox/MainWindow.xaml) | Use native controls, font icons, and visual-state resources. Fixed titlebar search widths do not suit this app's compact layout. |

The send transition and fades use Windows App SDK APIs directly, with no additional animation package. CommunityToolkit.Mvvm supplies `INotifyPropertyChanged` and `ICommand`; Rust owns message ordering and acknowledgement.

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

The Files follow-up passed `mise run check:windows` with zero warnings/errors and `mise run publish:windows`. The published app loaded `Themes/ChatResources.xbf`, passed the chat smoke checks, and accepted physical mouse clicks on search, new-chat, and send. UI Automation covered rapid sends, draft preservation, send/conversation interruptions, reduced motion, scroll-to-latest, and sending during animated scrolling. Desktop checks verified normal/maximized placement across relaunch, restoring normal bounds, a malformed settings file, an off-screen saved position, and closing while minimized. Twenty consecutive maximize/minimize/restore cycles completed before exercising toolbar search, new-chat focus, and sending. The disconnected-monitor case used an off-screen saved rectangle; physical monitor removal and mixed-DPI monitor transitions remain untested.

The modernization follow-up passed the Windows build and formatting checks with zero warnings or errors. UI Automation checked 12 consecutive sends, tail-following after layout, multiline text, draft preservation, 20 accepted-send/conversation-switch interruptions, and sending and sidebar toggling with Windows animations disabled. A physical mouse click verified the corrected latest-button hit area. A deterministic Draft adapter check covered a clear arriving before write completion, overlapping edits, initialization without a write, and retaining text after a failed write. The adapter rereads the authoritative projection after outstanding writes settle so an ignored in-flight revision cannot strand the editor on stale text.

The September 14, 2026 validation after the `Arut.Bindings` adoption passed `mise run surface:windows` and `mise run check:windows`, with zero build warnings or errors. UI Automation verified localized conversation and transcript labels, sending a message, receiving its echo, and clean window closure. `mise run publish:windows` also passed; the published app opened with localized resources and closed cleanly. Both Debug and Release tasks skipped unchanged binding generation on repeat runs. All 27 `arut-dev` tests, its Clippy check, and its generated-resource and architecture checks passed.

The Windows Release build passes with zero warnings. UI Automation verified three composer grow/clear cycles, independent drafts, and sending the final value after 30 consecutive edits. Keyboard input verified that Shift+Enter inserts a newline and Enter sends the exact two-line message. Twenty consecutive sends verified following the latest message and the jump-to-latest action after scrolling up.

Screenshots at 1100, 560, and 420 physical pixels checked wide, collapsed, and overlay layouts at 125% scaling. The compact caption and actions do not overlap. `DesktopTitleBar` corrects the pinned WinUI control's physical-pixel caption insets to logical units; [microsoft-ui-xaml#10344](https://github.com/microsoft/microsoft-ui-xaml/issues/10344) is still open, so the correction stays. Window closure completed after canceling native work. Multi-monitor DPI transitions, IME composition, and reduced-motion appearance still need interactive review on appropriate hardware/settings.

The floating-composer checks verified that the transcript extends behind the input and that the final message clears it after draft growth, clearing, and a narrow resize. Screenshots also checked the composer and jump-to-latest button over the middle of a scrolling transcript.
