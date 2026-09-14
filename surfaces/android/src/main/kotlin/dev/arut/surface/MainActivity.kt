package dev.arut.surface

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.material3.windowsizeclass.ExperimentalMaterial3WindowSizeClassApi
import androidx.compose.material3.windowsizeclass.calculateWindowSizeClass
import androidx.compose.runtime.remember
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory

class MainActivity : ComponentActivity() {
    @OptIn(ExperimentalMaterial3WindowSizeClassApi::class)
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        setContent {
            val sessions: SessionViewModel = viewModel()
            val factory = remember(sessions) {
                viewModelFactory { initializer { ConversationViewModel(sessions.session) } }
            }
            val conversations: ConversationViewModel = viewModel(factory = factory)
            // The window knows its own size class; the screen never measures one.
            val width = calculateWindowSizeClass(this).widthSizeClass
            ArutTheme { ChatScreen(conversations, width) }
        }
    }
}
