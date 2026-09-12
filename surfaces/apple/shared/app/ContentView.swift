import ArutBindings
import ArutSurface
import SwiftUI

struct ContentView: View {
    @StateObject private var model = ChatModel()

    var body: some View {
        ChatView(model: model)
    }
}
