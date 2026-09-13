import ArutBindings
import ArutSurface
import SwiftUI
struct ContentView: View {
    let session: ProductSessionHandle
    var body: some View { ChatView(session: session) }
}
