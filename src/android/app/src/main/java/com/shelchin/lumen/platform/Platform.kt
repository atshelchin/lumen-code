package com.shelchin.lumen.platform

import android.app.Activity
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import android.provider.MediaStore
import android.provider.OpenableColumns
import android.view.WindowManager
import androidx.core.content.FileProvider
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import com.shelchin.lumen.MainActivity
import java.io.File
import java.nio.ByteBuffer

/** Haptics the core asks for by name. */
class Haptics(context: Context) {
    private val vibrator: Vibrator? = if (Build.VERSION.SDK_INT >= 31) {
        context.getSystemService(VibratorManager::class.java)?.defaultVibrator
    } else {
        @Suppress("DEPRECATION")
        context.getSystemService(Vibrator::class.java)
    }

    fun play(kind: String) {
        val v = vibrator ?: return
        if (!v.hasVibrator()) return
        val effect = when (kind) {
            // one step stronger than the names say: many Android motors play TICK and CLICK
            // too faintly to notice in the hand (the lock-on "light" is the one that matters)
            "selection" -> VibrationEffect.createPredefined(VibrationEffect.EFFECT_CLICK)
            "light" -> VibrationEffect.createPredefined(VibrationEffect.EFFECT_HEAVY_CLICK)
            "medium" -> VibrationEffect.createPredefined(VibrationEffect.EFFECT_HEAVY_CLICK)
            "success" -> VibrationEffect.createPredefined(VibrationEffect.EFFECT_DOUBLE_CLICK)
            "warning" -> VibrationEffect.createWaveform(longArrayOf(0, 30, 80, 30), intArrayOf(0, 160, 0, 160), -1)
            "error" -> VibrationEffect.createWaveform(longArrayOf(0, 40, 60, 40, 60, 40), intArrayOf(0, 255, 0, 255, 0, 255), -1)
            else -> VibrationEffect.createPredefined(VibrationEffect.EFFECT_CLICK)
        }
        v.vibrate(effect)
    }
}

/** Screen kept on, full brightness and system bars hidden while a code is shown. */
class DeviceControl(private val activity: Activity) {
    fun keepAwake(on: Boolean) {
        if (on) activity.window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        else activity.window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
    }

    fun boost(on: Boolean) {
        val lp = activity.window.attributes
        // a window-level override: the system setting is untouched and comes back by itself
        lp.screenBrightness = if (on) 1f else WindowManager.LayoutParams.BRIGHTNESS_OVERRIDE_NONE
        activity.window.attributes = lp
    }

    fun immersive(on: Boolean) {
        val c = WindowCompat.getInsetsController(activity.window, activity.window.decorView)
        if (on) {
            c.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            c.hide(WindowInsetsCompat.Type.systemBars())
        } else {
            c.show(WindowInsetsCompat.Type.systemBars())
        }
        fastRefresh(on)
    }

    /**
     * The panel's fastest refresh rate while a code plays: many phones idle at 60 Hz, where a
     * 60 fps code has no vsync to spare and uneven rates (45, 50) flip unevenly.
     */
    private fun fastRefresh(on: Boolean) {
        val lp = activity.window.attributes
        lp.preferredDisplayModeId = if (on) {
            @Suppress("DEPRECATION")
            val display = if (Build.VERSION.SDK_INT >= 30) activity.display else activity.windowManager.defaultDisplay
            val now = display?.mode
            display?.supportedModes
                ?.filter { now == null || (it.physicalWidth == now.physicalWidth && it.physicalHeight == now.physicalHeight) }
                ?.maxByOrNull { it.refreshRate }?.modeId ?: 0
        } else {
            0
        }
        activity.window.attributes = lp
    }
}

object Files {
    class Rgba(val width: Int, val height: Int, val rgba: ByteArray)

    /** Reads a document (name + bytes) and hands it over. */
    fun load(context: Context, uri: Uri, use: (String, ByteArray) -> Unit): Boolean = runCatching {
        var name = uri.lastPathSegment?.substringAfterLast('/') ?: "file"
        context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
            if (c.moveToFirst()) c.getString(0)?.let { name = it }
        }
        val bytes = context.contentResolver.openInputStream(uri)!!.use { it.readBytes() }
        use(name, bytes)
        true
    }.getOrDefault(false)

    /** Decodes and downsamples an image to tightly packed RGBA. */
    fun decodeImage(context: Context, uri: Uri, maxSide: Int): Rgba? = runCatching {
        val src = ImageDecoder.createSource(context.contentResolver, uri)
        val bmp = ImageDecoder.decodeBitmap(src) { decoder, info, _ ->
            decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
            val (w, h) = info.size.width to info.size.height
            val k = maxSide.toFloat() / maxOf(w, h)
            if (k < 1f) decoder.setTargetSize((w * k).toInt().coerceAtLeast(1), (h * k).toInt().coerceAtLeast(1))
        }.let { if (it.config != Bitmap.Config.ARGB_8888) it.copy(Bitmap.Config.ARGB_8888, false) else it }
        val buf = ByteBuffer.allocate(bmp.width * bmp.height * 4)
        bmp.copyPixelsToBuffer(buf)
        Rgba(bmp.width, bmp.height, buf.array())
    }.getOrNull()

    private fun temp(context: Context, bytes: ByteArray, name: String): File {
        val dir = File(context.cacheDir, "received").apply { deleteRecursively(); mkdirs() }
        val safe = name.replace('/', '_').ifBlank { "file" }
        return File(dir, safe).apply { writeBytes(bytes) }
    }

    private fun uriFor(context: Context, file: File): Uri =
        FileProvider.getUriForFile(context, "${context.packageName}.files", file)

    fun share(activity: Activity, bytes: ByteArray, name: String, mime: String): Boolean = runCatching {
        val uri = uriFor(activity, temp(activity, bytes, name))
        val send = Intent(Intent.ACTION_SEND).setType(mime).putExtra(Intent.EXTRA_STREAM, uri).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        activity.startActivity(Intent.createChooser(send, name))
        true
    }.getOrDefault(false)

    fun open(activity: Activity, bytes: ByteArray, name: String, mime: String): Boolean = runCatching {
        val uri = uriFor(activity, temp(activity, bytes, name))
        val view = Intent(Intent.ACTION_VIEW).setDataAndType(uri, mime).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        activity.startActivity(Intent.createChooser(view, name))
        true
    }.getOrDefault(false)

    /** Images go to Pictures/Lumen; everything else through the system's save dialog. */
    fun save(activity: MainActivity, bytes: ByteArray, name: String, mime: String, done: (Boolean, Boolean) -> Unit) {
        if (mime.startsWith("image/") && mime != "image/svg+xml") {
            val ok = runCatching {
                val values = ContentValues().apply {
                    put(MediaStore.Images.Media.DISPLAY_NAME, name)
                    put(MediaStore.Images.Media.MIME_TYPE, mime)
                    put(MediaStore.Images.Media.RELATIVE_PATH, Environment.DIRECTORY_PICTURES + "/Lumen")
                }
                val uri = activity.contentResolver.insert(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, values)!!
                activity.contentResolver.openOutputStream(uri)!!.use { it.write(bytes) }
                true
            }.getOrDefault(false)
            done(ok, false)
            return
        }
        activity.createDocument(name, mime) { uri ->
            if (uri == null) {
                done(false, true)
            } else {
                val ok = runCatching { activity.contentResolver.openOutputStream(uri)!!.use { it.write(bytes) }; true }.getOrDefault(false)
                done(ok, false)
            }
        }
    }
}
