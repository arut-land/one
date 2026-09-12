package dev.arut.surface

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import dev.arut.bindings.ChatModel

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        setContent {
            val model = remember { ChatModel() }
            DisposableEffect(model) {
                onDispose(model::close)
            }
            ChatScreen(model)
        }
    }
}
