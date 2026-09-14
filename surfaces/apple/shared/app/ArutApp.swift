import ArutBindings
import ArutSurface
import SwiftUI

@main
struct ArutApp: App {
    // The app entry point is the composition root (ADR 0007): it constructs
    // the session once and hands it down to views, which never construct it.
    private let session = createProductSession(pendingScopeId: "local-demo")

    var body: some Scene {
        WindowGroup {
            ChatView(session: session)
                #if os(macOS)
                .frame(minWidth: 740, minHeight: 520)
                #endif
        }
        #if os(macOS)
        .defaultSize(width: 1080, height: 740)
        .windowToolbarStyle(.unified)
        .commands { ConversationCommands() }
        #endif
    }
}
