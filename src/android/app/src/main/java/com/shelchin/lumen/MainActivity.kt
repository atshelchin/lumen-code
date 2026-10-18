package com.shelchin.lumen

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContract
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import androidx.core.content.IntentCompat
import com.shelchin.lumen.core.AppModel
import com.shelchin.lumen.core.Ev
import com.shelchin.lumen.ui.Root

class MainActivity : ComponentActivity() {
    lateinit var model: AppModel
        private set

    private var onPermission: ((Boolean) -> Unit)? = null
    private val permission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        onPermission?.invoke(granted)
        onPermission = null
    }
    private var onPicked: ((Uri?) -> Unit)? = null
    private val openDocument = registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        onPicked?.invoke(uri)
        onPicked = null
    }
    private var onImage: ((Uri?) -> Unit)? = null
    private val pickMedia = registerForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        onImage?.invoke(uri)
        onImage = null
    }
    private var onCreated: ((Uri?) -> Unit)? = null
    private val createDocument = registerForActivityResult(CreateDocument()) { uri ->
        onCreated?.invoke(uri)
        onCreated = null
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.dark(android.graphics.Color.TRANSPARENT),
            navigationBarStyle = SystemBarStyle.dark(android.graphics.Color.TRANSPARENT),
        )
        super.onCreate(savedInstanceState)
        model = AppModel(this)
        handle(intent)
        setContent { Root(model) }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handle(intent)
    }

    override fun onStart() {
        super.onStart()
        model.dispatch(Ev.appActive(true))
    }

    override fun onStop() {
        model.dispatch(Ev.appActive(false))
        super.onStop()
    }

    /** "Send with Lumen" from another app. */
    private fun handle(intent: Intent?) {
        val uri = when (intent?.action) {
            Intent.ACTION_SEND -> IntentCompat.getParcelableExtra(intent, Intent.EXTRA_STREAM, Uri::class.java)
            Intent.ACTION_VIEW -> intent.data
            else -> null
        }
        if (uri != null) {
            model.open(uri)
            return
        }
        val text = intent?.takeIf { it.action == Intent.ACTION_SEND }?.getStringExtra(Intent.EXTRA_TEXT)
        if (!text.isNullOrEmpty()) {
            model.dispatch(Ev.navigate("send"))
            model.dispatch(Ev.sourceTab("text"))
            model.dispatch(Ev.textChanged(text))
            pendingText = text
        }
        intent?.getStringExtra("lumen.script")?.let { DebugScript.run(this, model, it) }
    }

    /** Text shared from another app, for the text box to pick up. */
    var pendingText: String? = null

    fun withCameraPermission(then: (Boolean) -> Unit) {
        if (ContextCompat.checkSelfPermission(this, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) {
            then(true)
        } else {
            onPermission = then
            permission.launch(Manifest.permission.CAMERA)
        }
    }

    fun pickFile(then: (Uri?) -> Unit) {
        onPicked = then
        openDocument.launch(arrayOf("*/*"))
    }

    fun pickImage(then: (Uri?) -> Unit) {
        onImage = then
        pickMedia.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))
    }

    fun createDocument(name: String, mime: String, then: (Uri?) -> Unit) {
        onCreated = then
        createDocument.launch(name to mime)
    }

    /** ACTION_CREATE_DOCUMENT with the file's own MIME type. */
    private class CreateDocument : ActivityResultContract<Pair<String, String>, Uri?>() {
        override fun createIntent(context: Context, input: Pair<String, String>) =
            Intent(Intent.ACTION_CREATE_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType(input.second)
                .putExtra(Intent.EXTRA_TITLE, input.first)

        override fun parseResult(resultCode: Int, intent: Intent?): Uri? = if (resultCode == RESULT_OK) intent?.data else null
    }
}
