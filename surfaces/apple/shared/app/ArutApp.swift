import ArutFfi
import SwiftUI

@main
struct ArutApp: App {
    // The app entry point is the composition root (ADR 0007): it constructs
    // the session once and hands it down to views, which never construct it.
    private let session = createProductSession(pendingScopeId: "local-demo")

    var body: some Scene {
        WindowGroup {
            ContentView(session: session)
        }
    }
}
