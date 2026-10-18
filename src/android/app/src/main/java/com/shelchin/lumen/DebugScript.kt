package com.shelchin.lumen

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint
import android.util.Log
import androidx.lifecycle.lifecycleScope
import com.shelchin.lumen.core.AppModel
import com.shelchin.lumen.core.Ev
import com.shelchin.lumen.core.WireJson
import com.shelchin.lumen.ui.sentFrames
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.jsonObject
import uniffi.lumen_ffi.lumenReplay
import uniffi.lumen_ffi.lumenSelftest
import java.io.ByteArrayOutputStream
import java.io.File

/**
 * Debug builds: drive the app from an intent extra, for device tests without touching it.
 *
 *     adb shell am start -n com.shelchin.lumencode/com.shelchin.lumen.MainActivity \
 *       --es lumen.script "nav:send;demo:40000;wait:2;tap"
 *
 * Commands as on iOS: nav, lang, tab, text, sendtext, demo, wait, tap, settings, play, camera,
 * close, json, image, capture, feed, bench, event, dump; plus flips:<seconds> (the sender's
 * real frame rate), cammode:<fps> (forces a camera band; `formats` lists what it allows),
 * watch:<seconds> (camera settings + receiver numbers each second) and grab:<n> (raw frames).
 * Results go to logcat (tag lumen-script) and, for bench/flips/dump/formats, to files/.
 */
object DebugScript {
    private var captured = mutableListOf<Triple<Int, Int, ByteArray>>()

    fun run(activity: MainActivity, model: AppModel, script: String) {
        if (!BuildConfig.SCRIPTS) return
        activity.lifecycleScope.launch {
            delay(600)
            for (raw in script.split(';')) {
                val cmd = raw.split(':', limit = 2)
                val arg = cmd.getOrElse(1) { "" }
                Log.i("lumen-script", raw)
                when (cmd[0]) {
                    "nav" -> model.dispatch(Ev.navigate(arg))
                    "lang" -> model.dispatch(Ev.toggleLang)
                    "tab" -> model.dispatch(Ev.sourceTab(arg))
                    "text" -> model.dispatch(Ev.textChanged(arg))
                    "sendtext" -> model.dispatch(Ev.sendText)
                    "demo" -> {
                        val n = arg.toIntOrNull() ?: 20000
                        var x = 0x9e3779b9.toInt()
                        val bytes = ByteArray(n) { x = x xor (x shl 13); x = x xor (x ushr 17); x = x xor (x shl 5); x.toByte() }
                        model.core.loadFile("demo-$n.bin", bytes)
                    }
                    "wait" -> delay(((arg.toDoubleOrNull() ?: 1.0) * 1000).toLong())
                    "tap" -> model.dispatch(Ev.tapCode)
                    "settings" -> model.dispatch(Ev.toggleSettings)
                    "play" -> model.dispatch(Ev.togglePlay)
                    "camera" -> model.dispatch(Ev.openCamera)
                    "close" -> model.dispatch(Ev.closeSend)
                    "json" -> {
                        model.dispatch(Ev.textChanged("""{"name":"Lumen Code","version":1,"big":12345678901234567890,"list":[1,2,3,{"deep":true}],"url":"https://example.com/a?b=1","note":"把文件变成流动的光","nothing":null}"""))
                        model.dispatch(Ev.sendText)
                    }
                    "image" -> {
                        val bmp = Bitmap.createBitmap(720, 480, Bitmap.Config.ARGB_8888)
                        val c = Canvas(bmp)
                        c.drawColor(0xFF0A0D14.toInt())
                        val p = Paint(Paint.ANTI_ALIAS_FLAG)
                        listOf(0xFF22D8FF, 0xFFFF3D9A, 0xFFFFD60A, 0xFF2BDC74).forEachIndexed { i, col ->
                            p.color = col.toInt(); c.drawCircle(120f + i * 160f, 240f, 64f, p)
                        }
                        val out = ByteArrayOutputStream()
                        bmp.compress(Bitmap.CompressFormat.PNG, 100, out)
                        model.core.loadFile("circles.png", out.toByteArray())
                    }
                    "capture" -> {
                        captured.clear()
                        repeat(arg.toIntOrNull() ?: 4) {
                            val f = model.core.sendFrame() ?: return@repeat
                            captured += upscale(f.rgba, f.width.toInt(), f.height.toInt(), 3)
                        }
                        Log.i("lumen-script", "captured ${captured.size} frames")
                    }
                    "feed" -> for ((w, h, rgba) in captured) {
                        model.core.scanImage(w.toUInt(), h.toUInt(), rgba)
                        delay(80)
                    }
                    "bench" -> {
                        val n = arg.toUIntOrNull() ?: 300u
                        val result = withContext(Dispatchers.Default) { lumenSelftest(n) }
                        Log.i("lumen-bench", result)
                        File(activity.filesDir, "bench.txt").writeText(result)
                    }
                    "event" -> runCatching { model.dispatch(WireJson.parseToJsonElement(arg).jsonObject) }
                    "dump" -> File(activity.filesDir, "view.json").writeText(model.core.view())
                    "flips" -> {
                        val secs = arg.toDoubleOrNull() ?: 3.0
                        val (n0, t0) = sentFrames() to System.nanoTime()
                        delay((secs * 1000).toLong())
                        val rate = (sentFrames() - n0) * 1e9 / (System.nanoTime() - t0)
                        val line = "sender %.1f frames/s (set %d) · display %.0f Hz".format(rate, model.view?.send?.fps ?: 0, activity.display?.refreshRate ?: 0f)
                        Log.i("lumen-script", line)
                        File(activity.filesDir, "flips.txt").appendText(line + "\n")
                    }
                    "cammode" -> model.camera.setMode(arg.toIntOrNull() ?: 30)
                    "formats" -> File(activity.filesDir, "formats.txt").writeText(model.camera.describe())
                    "grab" -> model.camera.grab(arg.toIntOrNull() ?: 30)
                    "replay" -> {
                        // raw frames (frame-N.nv12) in files/<arg> through the engine, on a
                        // foreground thread: what scanning those captures costs on this phone
                        val dir = File(activity.filesDir, arg.ifEmpty { "replay" }).path
                        val result = withContext(Dispatchers.Default) { lumenReplay(dir, 3u) }
                        Log.i("lumen-bench", result)
                        File(activity.filesDir, "replay.txt").appendText(result + "\n")
                    }
                    "watch" -> {
                        // once a second: the camera's settings and the receiver's numbers
                        val out = File(activity.filesDir, "watch.txt")
                        var grabs = 0
                        var lastUpdates = model.updates
                        var lastGrab = -100
                        repeat(arg.toIntOrNull() ?: 30) { sec ->
                            val r = model.view?.recv
                            // a few raw frames while a code is in view, for offline analysis
                            if (r?.live == true && grabs < 4 && sec - lastGrab >= 40) {
                                model.camera.grab(12)
                                grabs++
                                lastGrab = sec
                                out.appendText("grab $grabs\n")
                            }
                            val ups = model.updates - lastUpdates
                            lastUpdates = model.updates
                            val clock = java.text.SimpleDateFormat("HH:mm:ss", java.util.Locale.US).format(java.util.Date())
                            val line = "$clock ups $ups heat %.2f %.1f°C ${model.camera.stats()} | ".format(model.headroom, model.batteryC) + "${r?.diag} | ${r?.metrics?.joinToString { it.label + " " + it.value }} | ${r?.tip} | ${r?.pct}"
                            Log.i("lumen-watch", line)
                            out.appendText(line + "\n")
                            delay(1000)
                        }
                    }
                    else -> Log.w("lumen-script", "unknown command $raw")
                }
            }
        }
    }

    private fun upscale(src: ByteArray, w: Int, h: Int, k: Int): Triple<Int, Int, ByteArray> {
        val out = ByteArray(w * k * h * k * 4)
        for (y in 0 until h * k) for (x in 0 until w * k) {
            val si = ((y / k) * w + x / k) * 4
            val di = (y * w * k + x) * 4
            out[di] = src[si]; out[di + 1] = src[si + 1]; out[di + 2] = src[si + 2]; out[di + 3] = -1
        }
        return Triple(w * k, h * k, out)
    }
}
