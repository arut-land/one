package dev.arut.surface

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.remember
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        setContent {
            val sessions: SessionViewModel = viewModel()
            val factory = remember(sessions) {
                viewModelFactory { initializer { ConversationViewModel(sessions.session) } }
            }
            val conversations: ConversationViewModel = viewModel(factory = factory)
            ArutTheme { ChatScreen(conversations) }
        }
    }
}
