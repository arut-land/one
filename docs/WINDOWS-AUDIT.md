# Windows implementation audit

Reviewed the handwritten Windows view, presentation models, window lifecycle, motion helper, resources, and project configuration against the installed WinUI 2.3.6 templates and Microsoft guidance. Windows App SDK 2.4.0 supplies that WinUI component. This records implementation decisions and verification limits, not a certification of accessibility compliance.

| Area | Implementation and decision |
| --- | --- |
| Window and material | Native `Window`, `AppWindow`, `TitleBar`, caption buttons, and `MicaBackdrop` retain system window behavior. The compact titlebar uses the standard 32-DIP height. The overlay history pane uses `AcrylicInAppFillColorDefaultBrush`; the persistent wide pane lets the Mica base show through. No custom window frame or blur renderer. |
| History | `SplitView` owns pane layout and light dismiss. `DefaultListViewItemStyle` owns rounded selection, its accent indicator, hover, and focus. Arrow-key selection keeps focus in the list; explicit item activation moves to the composer. Existing rows update through `INotifyPropertyChanged` and move through `ObservableCollection.Move`. |
| Transcript | `ListView` and `ItemsStackPanel` retain virtualization and native keyboard focus. The custom item template was removed. Only hover fill is suppressed for non-selectable message rows. Item layout transitions remain disabled as a product choice. |
| Composer | A multiline `TextBox` owns measurement, selection, undo, wrapping, scrolling, and text scaling. `MinHeight`, `MaxHeight`, and bottom alignment replace a detached measuring `TextBlock` and manual resize handlers. Enter-to-send is the intentional chat shortcut; Shift+Enter and IME composition retain text-entry behavior. |
| Geometry and typography | Controls use native corner resources and caption/title styles. The panel uses `OverlayCornerRadius`; toolbar buttons inherit their control radius. The composer intentionally uses a 27-DIP radius for its single-line pill shape. Message bubbles and circular action buttons retain deliberate chat-specific shapes. Text uses native line metrics instead of a fixed line height. |
| Motion | Replies use `FadeInThemeAnimation`. The existing Toolkit transition matches composer and message backgrounds while leaving text unscaled. It uses the Toolkit's default easing, a short send duration, and checks the Windows animation preference. Composer focus uses the native focused text-control border brush while preserving the Acrylic material. There are no custom springs or item layout animations. |
| Contrast and focus | Theme resources supply colors. The composer has a stable border; contrast themes also give toolbar controls visible borders and hide decorative scroll fades. The composer card indicates focus; its inner TextBox's redundant system focus rectangle remains disabled. Standard controls keep their native focus treatment. |
| Errors and commands | Native `InfoBar`, row-attached `MenuFlyout`, `XamlUICommand`, and clipboard APIs handle platform interaction. New and Find share commands between their buttons and shortcuts; Copy uses `StandardUICommand`. Rust outcomes map to generated localized messages. View shortcuts call named actions directly rather than fabricating routed-event arguments. |
| State and shutdown | Rust owns chat and draft behavior. The Windows model owns display state and native handle lifetime. Ordered commands and cancellation drain before disposal; asynchronous window closing waits for that work. This lifetime code is required for native ownership. |

## Intentional custom code retained

- `DesktopTitleBar` converts caption insets from physical pixels to DIPs. Microsoft's [current `TitleBar::UpdatePadding` implementation](https://github.com/microsoft/microsoft-ui-xaml/blob/main/controls/dev/TitleBar/TitleBar.cpp) still assigns `AppWindowTitleBar` insets directly to XAML column widths. Recheck this when upgrading WinUI, then remove the subclass once the underlying conversion is fixed.
- The floating composer needs a measured scrolling footer so the last message clears it. Reading restoration stores an item and a local offset because virtualized global offsets are estimates. Native `ScrollIntoView`, `ChangeView`, and `ItemsUpdatingScrollMode` still perform the scrolling.
- The send coordinator waits for the destination container, releases replies on completion or cancellation, and has a deadline for an unrealized destination. This implements the requested linked send/reply interaction without reserving a blank reply area.
- The publish target includes loose XBF and PRI resources that the ordinary .NET publish pipeline does not collect. Removing it breaks the unpackaged app.

## State, organization, and tooling pass

`ChatView.xaml.cs` now coordinates input and conversation selection. Transcript
geometry and reading-position restoration live in `ChatView.Transcript.cs`;
history reconciliation lives in `ChatView.History.cs`. These remain parts of the
same view because they own XAML elements and local interaction state. Message and
history rows are separated from the model's native handle lifetime.

`ConversationCommands` coalesces only queued draft replacements. An in-flight
edit completes normally, and a send seals its preceding edit so subsequent typing
stays after that send. Draft versions prevent older completions from overwriting
new typing. Completion explicitly reconciles the cached draft even when the
observer saw the acknowledgment before the command finished. Draft-only commands
no longer read unrelated chat state after every replacement.

Rust now projects a bounded, single-line preview from the latest accepted message
into `ChatSummary`. The existing weak session callback updates initialized and
active conversations without per-chat tasks or copying entire transcripts.
Windows no longer derives previews from opened view models. History notifications
use structural equality, and reconciliation is coalesced while retaining row
identity. Stable rows avoid repeated linear searches. Reading-anchor selection
uses a single scan of realized containers; an extra layout/realization pass is
needed only when the final item has no container.

Initial and minimum window sizes are converted from DIPs to physical pixels, with
the initial size limited to the monitor work area. Minimum constraints track
scale changes. The sidebar's technical "Local session" footer was removed. Newly
inserted replies raise a native UI Automation notification without making the
whole transcript a live region or announcing recycled historical rows.

The build script selects x64 MSVC even when invoked from an x86 Developer Prompt.
`check:windows` now also checks generated binding facades, localized resources,
and dependency boundaries. Cargo/MSBuild retain ownership of incremental builds;
BoltFFI still comes from the pinned prebuilt tool installation.

Validation includes 31 focused Rust tests and strict Clippy, 11 .NET observer and
command tests, the full Windows check task, and native input/scroll/focus checks.
The command tests cover 1,000 queued draft edits, an intervening send, later typing,
and disposal waiting for pending work. Live UI checks confirm the latest accepted
reply appears in the sidebar preview. Multi-monitor DPI changes and Narrator
speech behavior still require manual verification.

## Cleanup verification

The final cleanup flattens transcript change handling, removes redundant windowing
namespace qualifiers, and uses one native-binding directory for copying and
missing-file diagnostics. XAML styles use `StaticResource`; brushes retain
`ThemeResource` so appearance changes still update them, following Microsoft's
[XAML resource guidance](https://learn.microsoft.com/en-us/windows/apps/develop/platform/xaml/xaml-theme-resources).
WinApp CLI was unavailable, so this pass used Microsoft documentation and the
existing native controls rather than CLI sample discovery.

| Review | Severity | Result |
| --- | --- | --- |
| `ChatView.xaml.cs`, transcript change handler | Note, fixed | Early returns remove nested event handling without changing notification or scroll behavior. |
| `ChatView.xaml`, `MessageBubble.xaml`, style references | Note, fixed | Static style lookup is consistent with the existing resource styles. |
| `Arut.Windows.csproj`, native bindings | Note, fixed | Build validation and DLL copying share the same configuration-specific path. |
| Motion | No new defect confirmed | Keep the documented send morph, native reply fade, cancellation, and Windows animation preference handling. No additional animation justified implementation. |

The reviewed chat flow scores 9/10 on the UX quick diagnostic. The remaining
minor issue is the shared internal-error message, "Something went wrong on your
node," which lacks a recovery action. Reaching 10/10 on this diagnostic requires
operation-specific recovery guidance that distinguishes a safe retry from an
already accepted send. That shared error-contract change is outside this cleanup.
The score covers the inspected chat flow, not untested accessibility settings.

This pass reran `check:windows`, all 31 focused Rust tests, and strict Clippy.
The Debug build had zero warnings or errors; all 11 .NET tests passed. Release
publishing succeeded, and the published executable opened a responsive Arut
window with its native DLL, PRI, and both XBF views present. Debug UI Automation
verified send/echo, updated sidebar preview, composer clearing, and independent
draft restoration across conversation switches. The final Release instance was
left running. Live contrast and reduced-motion changes, Narrator, and
multi-monitor DPI behavior remain manual checks.

## Platform references

- [Windows titlebar design](https://learn.microsoft.com/en-us/windows/apps/design/basics/titlebar-design)
- [Windows geometry](https://learn.microsoft.com/en-us/windows/apps/design/style/rounded-corner)
- [Keyboard interactions](https://learn.microsoft.com/en-us/windows/apps/develop/input/keyboard-interactions)
- [Contrast themes](https://learn.microsoft.com/en-us/windows/apps/design/accessibility/high-contrast-themes)
- [Motion in Windows](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/motion)

## Verification

The follow-up skill audit keeps the existing native control templates and disabled
item-layout transitions. Reply fades now use one XAML storyboard per realized
bubble, stopped on unload and item replacement. The existing Toolkit visual-tree
extensions replace the recursive helper. Send completion no longer takes focus
back from another control; invoking jump-to-latest moves focus to the composer
before its button disappears.

On Windows 10 build 19041 and newer, the
[animation preference change event](https://learn.microsoft.com/en-us/uwp/api/windows.ui.viewmanagement.uisettings.animationsenabledchanged?view=winrt-26100)
cancels pending send motion and clears reply reveal requests on the UI thread.
The subscription is version-gated and removed during disposal. Older systems keep
the existing preference check before sending. Transition cleanup releases held
replies even if resetting the Toolkit visuals fails. These changes follow the
[native theme animation guidance](https://learn.microsoft.com/en-us/windows/apps/develop/motion/xaml-animation)
without adding another animation dependency.

Native UI Automation and keyboard checks cover repeated composer growth and clearing, rapid edits, independent drafts, arrow navigation, Enter activation, Enter/Shift+Enter input, following the latest message, restoring reading position, and floating-composer clearance at narrow widths. Build and formatting checks cover the changed XAML and C#.

The follow-up run passed three composer fill/clear cycles, 30 consecutive draft
replacements, independent draft restoration, multiline keyboard input, 20 sends
with scroll restoration and composer clearance, and four sends retaining focus
on the new-chat button after focus was moved there. The Windows build reported
zero warnings and errors; all eight .NET observer tests and formatting checks
passed. Live animation-preference switching remains a manual verification item.

Contrast-theme resources were reviewed in code. Full Narrator testing, live contrast-theme switching, touch/pen, IME language coverage, and the range of Windows text-scale settings remain manual checks. Do not infer those results from keyboard automation or a screenshot at one DPI.

## Material and version choices

[Mica](https://learn.microsoft.com/en-us/windows/apps/design/style/mica) is the persistent window base, visible around the content and in the titlebar. `LayerFillColorDefaultBrush` supplies the translucent content layer above it. [In-app Acrylic](https://learn.microsoft.com/en-us/windows/apps/develop/ui/materials) belongs to the transient overlay history pane. Menus and tooltips keep their native template materials. The floating composer and jump-to-latest control also use native in-app Acrylic. The bottom scroll fade is deliberately weaker than the top fade so the material can sample the transcript underneath. The composer's border changes to the native focused text-control brush without changing its size or replacing its material. Native materials own contrast, transparency-disabled, and unsupported-system fallbacks.

As checked on September 13, 2026, Windows App SDK 2.4.0 is the latest stable NuGet release; 2.4.1 is experimental. The existing TransitionHelper 0.1.260617-build.2640 is the newest version on its Toolkit Labs feed and remains a prerelease dependency. No new UI framework or animation package was added. The build SDK remains 26100 and the declared minimum remains Windows 10 build 17763. Windows 10 execution has not been verified locally; framework back-compatibility does not establish support for every dependency or system configuration.
