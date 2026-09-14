import ArutBindings
import SwiftUI

/// The settings pane `⌘,` opens (macOS rule 1.1). It shows what this session
/// can actually do: the node's capability manifest, as the availability scope
/// publishes it. Nothing here is a stored preference yet, so nothing here
/// pretends to be one.
public struct SettingsView: View {
    private let handle: AvailabilityHandle
    @State private var availability: Projection<SessionAvailability>

    @MainActor
    public init(session: ProductSessionHandle) {
        let handle = session.availability()
        self.handle = handle
        _availability = State(initialValue: Projection { handle.state() })
    }

    public var body: some View {
        Form {
            LabeledContent(L10n.labelNodeAvailability()) {
                Text(caption).foregroundStyle(.secondary)
            }
            LabeledContent(L10n.appName()) {
                Text(L10n.chatLocalSession()).foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
        .frame(minWidth: 380)
        .task { await availability.follow(handle.availabilityChanges()) }
    }

    private var caption: String {
        switch availability.value.composer {
        case .unknown: L10n.availabilityUnknown()
        case .available: L10n.availabilityAvailable()
        case .reportedUnavailable: L10n.availabilityReportedUnavailable()
        case .notAdvertised: L10n.availabilityNotAdvertised()
        case .manifestUnreachable: L10n.availabilityManifestUnreachable()
        }
    }
}
