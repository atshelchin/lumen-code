package com.shelchin.lumen.core

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.shelchin.lumen.MainActivity
import com.shelchin.lumen.platform.DeviceControl
import com.shelchin.lumen.platform.Files
import com.shelchin.lumen.platform.Haptics
import com.shelchin.lumen.platform.LumenCamera
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import uniffi.lumen_ffi.LumenCore
import uniffi.lumen_ffi.ShellListener
import uniffi.lumen_ffi.lumenLabels
import java.util.Locale
import java.util.concurrent.Executors

/**
 * The shell's half of the Crux bridge: forwards events to the core, shows its view, and
 * performs the operations it asks for. It decides nothing.
 */
class AppModel(private val activity: MainActivity) {
    var view by mutableStateOf<ViewModel?>(null)
        private set
    var labels by mutableStateOf<Map<String, String>>(emptyMap())
        private set
    /**
     * The code's outline while scanning, at the core's full update rate. Everything else on the
     * receive screen follows [view], which then refreshes at most 8 times a second: numbers
     * that change 20 times a second cannot be read, and recomposing the screen is a good part
     * of the work on a slow phone.
     */
    var overlay by mutableStateOf<OverlayVM?>(null)
        private set
    private var latest: ViewModel? = null
    private var shownAt = 0L
    private var showPending = false

    /**
     * Whether the outline changed visibly: a held phone moves the corners a few pixels between
     * scans, and every redraw of the full-screen outline costs a frame.
     */
    private fun moved(a: OverlayVM?, b: OverlayVM?): Boolean {
        if (a == null || b == null) return a !== b
        if (a.tone != b.tone || a.frameW != b.frameW || a.frameH != b.frameH || a.rotation != b.rotation) return true
        for (i in a.points.indices) {
            val p = a.points[i]
            val q = b.points.getOrNull(i) ?: return true
            if (kotlin.math.abs(p[0] - q[0]) > 4f || kotlin.math.abs(p[1] - q[1]) > 4f) return true
        }
        return false
    }

    /** Shows `v` now, or within 125 ms when only the scanning numbers changed. */
    private fun show(v: ViewModel) {
        latest = v
        val old = view
        val now = android.os.SystemClock.uptimeMillis()
        val wait = 125 - (now - shownAt)
        val scanning = v.screen == "receive" && v.recv.phase == "scanning"
        val calm = old != null && scanning && old.screen == v.screen && old.recv.phase == v.recv.phase &&
            old.recv.status == v.recv.status && old.recv.error == v.recv.error && old.recv.cameraOn == v.recv.cameraOn &&
            old.recv.zoom == v.recv.zoom && old.recv.note == v.recv.note && old.lang == v.lang
        if (!calm || wait <= 0) {
            if (v != old) view = v
            shownAt = now
        } else if (!showPending) {
            showPending = true
            main.postDelayed({ showPending = false; latest?.let { if (it != view) show(it) } }, wait)
        }
    }

    private val main = Handler(Looper.getMainLooper())
    private val io = Executors.newSingleThreadExecutor()
    private var labelsLang = ""
    private val prefs = activity.getSharedPreferences("lumen", Context.MODE_PRIVATE)
    val device = DeviceControl(activity)
    val haptics = Haptics(activity)

    val core: LumenCore = LumenCore(object : ShellListener {
        override fun onUpdate(updateJson: String) {
            // any thread: decoded here, applied on the main thread, in order
            val update = try {
                WireJson.decodeFromString(UpdateWire.serializer(), updateJson)
            } catch (e: Exception) {
                Log.w("lumen", "undecodable update: $e")
                return
            }
            main.post { apply(update) }
        }
    })
    val camera = LumenCamera(activity, core)

    init {
        dispatch(Ev.start(Locale.getDefault().toLanguageTag()))
        // heat: the core scans less and gives up the fast camera modes before the phone throttles
        activity.getSystemService(android.os.PowerManager::class.java)?.let { pm ->
            val level = { s: Int ->
                when (s) {
                    android.os.PowerManager.THERMAL_STATUS_NONE -> 0
                    android.os.PowerManager.THERMAL_STATUS_LIGHT, android.os.PowerManager.THERMAL_STATUS_MODERATE -> 1
                    android.os.PowerManager.THERMAL_STATUS_SEVERE -> 2
                    else -> 3
                }
            }
            statusLevel = level(pm.currentThermalStatus)
            pm.addThermalStatusListener(activity.mainExecutor) { statusLevel = level(it); sendThermal() }
            pollHeadroom(pm)
        }
        // and the battery's temperature, which every phone reports (some return no headroom)
        activity.registerReceiver(object : android.content.BroadcastReceiver() {
            override fun onReceive(c: Context, i: Intent) {
                val t = i.getIntExtra(android.os.BatteryManager.EXTRA_TEMPERATURE, Int.MIN_VALUE)
                if (t == Int.MIN_VALUE) return
                batteryC = t / 10f
                // up at 44 / 47 / 50 °C (a phone charging while in use sits around 40–45), down
                // a degree below
                val up = when {
                    batteryC >= 50f -> 3
                    batteryC >= 47f -> 2
                    batteryC >= 44f -> 1
                    else -> 0
                }
                val down = when {
                    batteryC >= 49f -> 3
                    batteryC >= 46f -> 2
                    batteryC >= 43f -> 1
                    else -> 0
                }
                batteryLevel = when {
                    up > batteryLevel -> up
                    down < batteryLevel -> down
                    else -> batteryLevel
                }
                sendThermal()
            }
        }, android.content.IntentFilter(Intent.ACTION_BATTERY_CHANGED))
        runCatching { accept(WireJson.decodeFromString(ViewModel.serializer(), core.view())) }
    }

    // heat, from three sources: the thermal status (which some vendors leave at "none" until
    // they throttle on their own), the headroom forecast (1.0 = throttling) and the battery
    private var statusLevel = 0
    private var headroomLevel = 0
    private var batteryLevel = 0
    /** Debug: the battery's temperature, °C. */
    var batteryC = Float.NaN
        private set
    private var sentLevel = -1
    /** Debug: the last headroom reading. */
    var headroom = Float.NaN
        private set

    private fun sendThermal() {
        val l = maxOf(statusLevel, headroomLevel, batteryLevel)
        if (l != sentLevel) {
            sentLevel = l
            dispatch(Ev.thermal(l))
        }
    }

    private fun pollHeadroom(pm: android.os.PowerManager) {
        if (android.os.Build.VERSION.SDK_INT >= 30) {
            val h = pm.getThermalHeadroom(10)
            if (!h.isNaN()) {
                headroom = h
                // up at 0.7 / 0.85 / 0.95, down 0.05 below (no flapping at a threshold)
                val up = when {
                    h >= 0.95f -> 3
                    h >= 0.85f -> 2
                    h >= 0.7f -> 1
                    else -> 0
                }
                val down = when {
                    h >= 0.9f -> 3
                    h >= 0.8f -> 2
                    h >= 0.65f -> 1
                    else -> 0
                }
                headroomLevel = when {
                    up > headroomLevel -> up
                    down < headroomLevel -> down
                    else -> headroomLevel
                }
            }
        }
        sendThermal()
        main.postDelayed({ pollHeadroom(pm) }, 5000)
    }

    fun t(key: String): String = labels[key] ?: key

    fun dispatch(event: Event) {
        try {
            core.dispatch(event.toString())
        } catch (e: Exception) {
            Log.w("lumen", "rejected event $event: $e")
        }
    }

    /** Debug: updates applied so far. */
    var updates = 0L
        private set

    private fun apply(update: UpdateWire) {
        updates++
        if (update.view.rev >= (view?.rev ?: 0)) accept(update.view)
        for (r in update.requests) perform(r)
    }

    private fun accept(v: ViewModel) {
        if (v.lang != labelsLang) {
            labelsLang = v.lang
            labels = runCatching {
                WireJson.parseToJsonElement(lumenLabels(v.lang)).jsonObject.mapValues { it.value.jsonPrimitive.content }
            }.getOrDefault(emptyMap())
        }
        val o = if (v.screen == "receive") v.recv.overlay else null
        if (moved(overlay, o)) overlay = o
        show(v)
        // the code's position steers the camera's metering and focus
        if (v.recv.cameraOn) {
            val o = v.recv.overlay
            camera.track(o?.points, o?.frameW ?: 0, o?.frameH ?: 0)
        }
    }

    fun answer(id: Long, output: JsonObject) {
        if (id == 0L) return
        runCatching { core.resolve(id.toULong(), output.toString()) }
    }

    private fun finished(ok: Boolean, cancelled: Boolean = false) = buildJsonObject {
        put("type", "finished"); put("ok", ok); put("cancelled", cancelled)
    }

    // ---------------------------------------------------------------- operations

    private fun perform(r: RequestWire) {
        val op = r.op
        when (op.type) {
            "load_prefs" -> answer(r.id, buildJsonObject {
                put("type", "prefs")
                put("json", prefs.getString("prefs", null)?.let { JsonPrimitive(it) } ?: JsonNull)
            })
            "save_prefs" -> prefs.edit().putString("prefs", op.json).apply()
            "keep_awake" -> device.keepAwake(op.on == true)
            "brightness" -> device.boost(op.boost == true)
            "immersive" -> device.immersive(op.on == true)
            "haptic" -> haptics.play(op.kind ?: "light")
            "camera_start" -> {
                val id = r.id
                activity.withCameraPermission { granted ->
                    if (!granted) {
                        answer(id, buildJsonObject { put("type", "camera"); put("ok", false); put("error", "denied") })
                    } else {
                        camera.start(op.device ?: 0) { out -> main.post { answer(id, out) } }
                    }
                }
            }
            "camera_stop" -> camera.stop()
            "camera_zoom" -> camera.zoom(op.level ?: 1f)
            "camera_mode" -> camera.setMode(op.fps ?: 30) // no 4K: the camera answers uhd false
            "camera_exposure" -> camera.setExposureCap(op.maxUs ?: 2500)
            "pick_file" -> {
                val id = r.id
                activity.pickFile { uri ->
                    if (uri == null) {
                        answer(id, finished(false, cancelled = true))
                    } else {
                        io.execute {
                            val ok = Files.load(activity, uri) { name, bytes -> core.loadFile(name, bytes) }
                            main.post { answer(id, finished(ok)) }
                        }
                    }
                }
            }
            "pick_image" -> {
                val id = r.id
                activity.pickImage { uri ->
                    if (uri == null) {
                        answer(id, finished(false, cancelled = true))
                    } else {
                        io.execute {
                            val img = Files.decodeImage(activity, uri, 2400)
                            if (img != null) core.scanImage(img.width.toUInt(), img.height.toUInt(), img.rgba)
                            main.post { answer(id, finished(img != null)) }
                        }
                    }
                }
            }
            "save_file", "share_file", "open_file" -> {
                val id = r.id
                val name = op.name ?: "file"
                val mime = op.mime ?: "application/octet-stream"
                val bytes = core.receivedBytes()
                if (bytes == null) {
                    answer(id, finished(false)); return
                }
                when (op.type) {
                    "save_file" -> Files.save(activity, bytes, name, mime) { ok, cancelled -> answer(id, finished(ok, cancelled)) }
                    "share_file" -> answer(id, finished(Files.share(activity, bytes, name, mime)))
                    else -> answer(id, finished(Files.open(activity, bytes, name, mime)))
                }
            }
            "copy_text" -> {
                val text = core.copyText(op.pretty ?: true)
                if (text != null) {
                    val cm = activity.getSystemService(ClipboardManager::class.java)
                    cm.setPrimaryClip(ClipData.newPlainText("Lumen", text))
                }
                answer(r.id, finished(text != null))
            }
            "open_url" -> op.url?.let { runCatching { activity.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(it))) } }
            "open_settings" -> runCatching {
                activity.startActivity(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", activity.packageName, null)))
            }
            else -> {
                Log.w("lumen", "unknown operation ${op.type}")
                answer(r.id, finished(false))
            }
        }
    }

    /** A file shared to Lumen from another app. */
    fun open(uri: Uri) {
        io.execute { Files.load(activity, uri) { name, bytes -> core.loadFile(name, bytes) } }
    }
}
