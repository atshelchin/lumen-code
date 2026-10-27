package com.shelchin.lumen.core

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

// Kotlin mirrors of the core's view model (src/app/core/src/app/view.rs) and of the update
// envelope (src/app/core/src/runtime.rs). Field names are the core's camelCase JSON.

val WireJson = Json { ignoreUnknownKeys = true; explicitNulls = false }

@Serializable
data class UpdateWire(val view: ViewModel, val requests: List<RequestWire> = emptyList())

/** id 0 = a notification: perform, never answer. */
@Serializable
data class RequestWire(val id: Long, val op: OpWire)

/** Every shell operation, flattened: `type` says which one, the rest are its fields. */
@Serializable
data class OpWire(
    val type: String,
    val json: String? = null,
    val on: Boolean? = null,
    val boost: Boolean? = null,
    val kind: String? = null,
    val device: Int? = null,
    val level: Float? = null,
    val fps: Int? = null,
    val uhd: Boolean? = null,
    val maxUs: Int? = null,
    val name: String? = null,
    val mime: String? = null,
    val pretty: Boolean? = null,
    val url: String? = null,
)

@Serializable
data class ViewModel(
    val rev: Long,
    val lang: String,
    val platform: String,
    val screen: String,
    val send: SendVM,
    val recv: RecvVM,
    val build: String = "",
)

@Serializable
data class StatVM(val label: String, val value: String, val wide: Boolean)

@Serializable
data class MetricVM(val label: String, val value: String)

@Serializable
data class GeometryVM(val s: Int, val x: Int, val y: Int, val widthPx: Int, val heightPx: Int)

@Serializable
data class SendVM(
    val phase: String,
    val sourceTab: String,
    val textMeta: String,
    val canSendText: Boolean,
    val dropDesc: String,
    val dropHint: String,
    val flashWarn: String = "",
    /** The code is ready but paused until the photosensitivity warning is accepted (`AcceptFlash`). */
    val flashAsk: Boolean = false,
    /** The Text tab's text: the field shows this, so what is shown is what is sent. */
    val text: String = "",
    val title: String,
    /** Bar, under the title: size and time per pass ("78 KB · ≈ 3 s"), or "Paused". */
    val subtitle: String = "",
    /** Settings panel: raw rate, per frame, grid, bits per cell, engine time, in one line. */
    val details: String = "",
    val size: String,
    val stats: List<StatVM>,
    val playing: Boolean,
    val fps: Int,
    val dockVisible: Boolean,
    val settingsOpen: Boolean,
    val density: String,
    val mode: Int,
    val ecc: Int,
    val geometry: GeometryVM? = null,
    val frameRev: Long,
    val error: String? = null,
)

@Serializable
data class ZoomVM(val level: Float, val min: Float, val max: Float, val label: String, val active: Boolean)

@Serializable
data class OverlayVM(val points: List<List<Float>>, val frameW: Int, val frameH: Int, val rotation: Int, val tone: String)

@Serializable
data class DoneVM(
    val kind: String? = null,
    val reader: Boolean,
    val name: String,
    val title: String,
    val sub: String,
    val stats: List<MetricVM>,
    val codeLine: String? = null,
    val statsOpen: Boolean,
    val json: Boolean,
    val pretty: Boolean,
    val wrap: Boolean,
    val font: Int,
    val copyLabel: String? = null,
    val link: String? = null,
    val saveLabel: String,
    val savePrimary: Boolean,
    val canOpen: Boolean,
    val note: String? = null,
    val noteOk: Boolean,
    val contentRev: Long,
)

@Serializable
data class RecvVM(
    val phase: String,
    val cameraOn: Boolean,
    val cameraStarting: Boolean,
    val error: String? = null,
    val canOpenSettings: Boolean,
    val note: String? = null,
    val live: Boolean,
    val status: String,
    val name: String,
    val pct: String,
    val progress: Float,
    /** Under the progress bar: "32 KB/s · 2s left" (empty until data arrives). */
    val line: String = "",
    /** Speed, time left, cell size, success: for the Details area. */
    val metrics: List<MetricVM>,
    val tip: String,
    val tipOk: Boolean,
    val diag: String,
    val zoom: ZoomVM? = null,
    val canSwitch: Boolean,
    val overlay: OverlayVM? = null,
    val done: DoneVM? = null,
)

/** Events, as the core's JSON vocabulary (src/app/core/src/app/mod.rs `Event`). */
object Ev {
    private fun e(type: String) = buildJsonObject { put("type", type) }
    fun start(locale: String) = buildJsonObject { put("type", "start"); put("locale", locale); put("platform", "android") }
    fun navigate(screen: String) = buildJsonObject { put("type", "navigate"); put("screen", screen) }
    val toggleLang = e("toggle_lang")
    fun viewport(w: Double, h: Double, scale: Double, mmPerPx: Double) = buildJsonObject {
        put("type", "viewport"); put("widthPx", w); put("heightPx", h); put("scale", scale); put("mmPerPx", mmPerPx)
    }
    fun appActive(on: Boolean) = buildJsonObject { put("type", "app_active"); put("active", on) }
    fun thermal(level: Int) = buildJsonObject { put("type", "thermal"); put("level", level) }
    fun sourceTab(t: String) = buildJsonObject { put("type", "source_tab"); put("tab", t) }
    fun textChanged(t: String) = buildJsonObject { put("type", "text_changed"); put("text", t) }
    val pickFile = e("pick_file")
    val sendText = e("send_text")
    fun density(v: String) = buildJsonObject { put("type", "set_density"); put("value", v) }
    fun mode(v: Int) = buildJsonObject { put("type", "set_mode"); put("value", v) }
    fun ecc(v: Int) = buildJsonObject { put("type", "set_ecc"); put("value", v) }
    fun fps(v: Int) = buildJsonObject { put("type", "set_fps"); put("value", v) }
    val togglePlay = e("toggle_play")
    val acceptFlash = e("accept_flash")
    val toggleSettings = e("toggle_settings")
    val tapCode = e("tap_code")
    val hideDock = e("hide_dock")
    val dockActivity = e("dock_activity")
    val closeSend = e("close_send")
    val openCamera = e("open_camera")
    val switchCamera = e("switch_camera")
    fun zoom(level: Float) = buildJsonObject { put("type", "set_zoom"); put("level", level) }
    val resetZoom = e("reset_zoom")
    val pickImage = e("pick_image")
    val receiveAnother = e("receive_another")
    val copy = e("copy")
    val openLink = e("open_link")
    val save = e("save")
    val share = e("share")
    val openFile = e("open_file")
    val openSettings = e("open_settings")
    fun jsonView(pretty: Boolean) = buildJsonObject { put("type", "set_json_view"); put("pretty", pretty) }
    val toggleWrap = e("toggle_wrap")
    fun font(d: Int) = buildJsonObject { put("type", "set_font"); put("delta", d) }
    val toggleStats = e("toggle_stats")
}

typealias Event = JsonObject
