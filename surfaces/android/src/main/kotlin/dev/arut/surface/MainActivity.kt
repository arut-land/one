package dev.arut.surface

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.material3.adaptive.currentWindowAdaptiveInfo
import androidx.lifecycle.viewmodel.compose.viewModel

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        setContent {
            val sessions: SessionViewModel = viewModel()
            // `viewModel { }` is the factory: no ViewModelProvider.Factory of
            // our own and no `remember` holding one alive.
            val conversations: ConversationViewModel =
                viewModel { ConversationViewModel(sessions.session) }
            // The window knows its own size class; the screen never measures one.
            val windowSizeClass = currentWindowAdaptiveInfo().windowSizeClass
            ArutTheme { ChatScreen(conversations, windowSizeClass) }
        }
    }
}
