package com.shelchin.lumen.ui

import android.graphics.Bitmap
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.shelchin.lumen.BuildConfig
import com.shelchin.lumen.MainActivity
import com.shelchin.lumen.core.AppModel
import com.shelchin.lumen.core.Ev
import com.shelchin.lumen.core.GeometryVM
import com.shelchin.lumen.core.SendVM
import com.shelchin.lumen.core.StatVM
import com.shelchin.lumen.core.ViewModel
import java.nio.ByteBuffer
import java.util.concurrent.Executors
import java.util.concurrent.Future
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
import kotlin.math.abs
import kotlin.math.roundToInt
import kotlinx.coroutines.isActive
import uniffi.lumen_ffi.LumenCore

/** The sender (the web's send.html): pick a file or type text, then the code fills the screen. */
@Composable
fun SendScreen(model: AppModel, v: ViewModel) {
    val s = v.send
    val context = LocalContext.current
    Box(Modifier.fillMaxSize()) {
        BoxWithConstraints(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
            val density = LocalDensity.current.density
            val w = constraints.maxWidth
            val h = constraints.maxHeight
            LaunchedEffect(w, h) {
                val dm = context.resources.displayMetrics
                // physical pixel size; some devices report a nonsense xdpi, fall back to the density bucket
                val dpi = if (abs(dm.xdpi - dm.densityDpi) / dm.densityDpi < 0.3f) dm.xdpi else dm.densityDpi.toFloat()
                model.dispatch(Ev.viewport(w.toDouble(), h.toDouble(), density.toDouble(), 25.4 / dpi))
            }
            if (s.phase == "pick") {
                PickCard(model, s)
            } else {
                Box(Modifier.fillMaxSize().background(Lumen.ink))
                s.geometry?.let { CodeView(model.core, it, s.frameRev, s.playing, s.fps) { model.dispatch(Ev.tapCode) } }
            }
        }
        if (s.phase == "pick") {
            Box(Modifier.windowInsetsPadding(WindowInsets.safeDrawing).padding(horizontal = 10.dp, vertical = 8.dp)) {
                HomeButton { model.dispatch(Ev.navigate("home")) }
            }
        } else {
            AnimatedVisibility(
                visible = s.dockVisible,
                enter = fadeIn() + slideInVertically { it / 6 },
                exit = fadeOut() + slideOutVertically { it / 6 },
                modifier = Modifier.align(Alignment.BottomCenter),
            ) {
                Dock(model, s)
            }
        }
    }
}

/**
 * The animated code. Per frame only a one-pixel-per-cell bitmap changes (a few thousand
 * pixels); the GPU scales it up with nearest-neighbour sampling and draws the static mask
 * (finders, markers, rounded gaps) on top. No per-pixel work at screen resolution.
 */
@Composable
private fun CodeView(core: LumenCore, g: GeometryVM, frameRev: Long, playing: Boolean, fps: Int, onTap: () -> Unit) {
    val frames = remember { CodeFrames(core) }
    var tick by remember { mutableIntStateOf(0) }
    LaunchedEffect(frameRev, g) {
        frames.reload(g)
        tick++
    }
    LaunchedEffect(playing, fps, frameRev) {
        if (!playing) return@LaunchedEffect
        val period = 1_000_000_000L / fps.coerceAtLeast(1)
        var next = 0L
        while (isActive) {
            withFrameNanos { now ->
                if (next == 0L) next = now
                // flip on the vsync closest to the schedule; keep the cadence. A frame still
                // being built is shown on the next vsync instead of being skipped.
                if (now + 4_000_000L >= next && frames.flip()) {
                    next = maxOf(next + period, now + period / 2)
                    tick++
                }
            }
        }
    }
    DisposableEffect(Unit) { onDispose { frames.release() } }
    Canvas(Modifier.fillMaxSize().pointerInput(Unit) { detectTapGestures { onTap() } }) {
        @Suppress("UNUSED_EXPRESSION") tick
        val cells = frames.front ?: return@Canvas
        val mask = frames.mask ?: return@Canvas
        val ox = ((size.width - g.widthPx) / 2f).roundToInt()
        val oy = ((size.height - g.heightPx) / 2f).roundToInt()
        drawImage(cells, srcSize = IntSize(cells.width, cells.height), dstOffset = IntOffset(ox, oy), dstSize = IntSize(g.widthPx, g.heightPx), filterQuality = FilterQuality.None)
        drawImage(mask, srcSize = IntSize(mask.width, mask.height), dstOffset = IntOffset(ox, oy), dstSize = IntSize(g.widthPx, g.heightPx), filterQuality = FilterQuality.None)
    }
}

/**
 * A ring of three cell bitmaps and the mask. The next frame is built on a background thread
 * while the current one is on screen, into the bitmap shown two flips ago: the render thread,
 * which draws a frame after the main thread has moved on, never reads a bitmap being written.
 */
private class CodeFrames(private val core: LumenCore) {
    private var bitmaps = arrayOfNulls<Bitmap>(3)
    private var images = arrayOfNulls<ImageBitmap>(3)
    private var shown = 0
    private val ready = AtomicBoolean(false)
    private var pending: Future<*>? = null
    private val builder = Executors.newSingleThreadExecutor { r -> Thread(r, "lumen-frames").apply { priority = Thread.MAX_PRIORITY } }
    var mask: ImageBitmap? = null
        private set
    val front: ImageBitmap? get() = images[shown]

    fun reload(g: GeometryVM) {
        settle()
        core.sendMask()?.let { m ->
            val bmp = Bitmap.createBitmap(m.width.toInt(), m.height.toInt(), Bitmap.Config.ARGB_8888)
            bmp.copyPixelsFromBuffer(ByteBuffer.wrap(m.rgba))
            mask = bmp.asImageBitmap()
        }
        val (w, h) = (g.x + 4) to (g.y + 4)
        if (bitmaps[0]?.width != w || bitmaps[0]?.height != h) {
            bitmaps = Array(3) { Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888) }
            images = Array(3) { bitmaps[it]!!.asImageBitmap() }
        }
        // the first frame of a new code at once, the next one in the background
        ready.set(build(next()))
        flip()
    }

    private fun next() = (shown + 1) % 3

    /** Fills bitmap `i` with the sender's next frame. */
    private fun build(i: Int): Boolean {
        val f = core.sendFrame() ?: return false
        val bmp = bitmaps[i] ?: return false
        if (f.width.toInt() != bmp.width || f.height.toInt() != bmp.height) return false
        bmp.copyPixelsFromBuffer(ByteBuffer.wrap(f.rgba))
        return true
    }

    /** Shows the frame built ahead, if it is done, and starts building the one after. */
    fun flip(): Boolean {
        if (!ready.get()) {
            if (pending?.isDone != false) buildAhead() // the last build failed: try again
            return false
        }
        shown = next()
        ready.set(false)
        buildAhead()
        if (BuildConfig.SCRIPTS) flips.incrementAndGet()
        return true
    }

    private fun buildAhead() {
        val i = next()
        pending = builder.submit { if (build(i)) ready.set(true) }
    }

    private fun settle() {
        runCatching { pending?.get() }
        pending = null
    }

    fun release() {
        settle()
        builder.shutdown()
        bitmaps.forEach { it?.recycle() }
    }

    companion object {
        /** Frames shown so far (debug builds), for the `flips` script command. */
        val flips = AtomicLong()
    }
}

/** Frames the sender has shown (debug builds). */
fun sentFrames(): Long = CodeFrames.flips.get()

@Composable
private fun PickCard(model: AppModel, s: SendVM) {
    val activity = LocalContext.current as MainActivity
    var text by remember { mutableStateOf(activity.pendingText ?: "") }
    var focused by remember { mutableStateOf(false) }
    val focus = LocalFocusManager.current
    val clipboard = LocalClipboardManager.current
    Column(
        Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp, vertical = 64.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Column(
            Modifier
                .widthIn(max = 520.dp)
                .fillMaxWidth()
                .drawBehind {
                    val r = 24.dp.toPx()
                    drawRoundRect(Color.White.copy(alpha = 0.02f), cornerRadius = CornerRadius(r))
                    drawRoundRect(
                        Color.White.copy(alpha = 0.16f), cornerRadius = CornerRadius(r),
                        style = Stroke(width = 1.5.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(6.dp.toPx(), 5.dp.toPx()))),
                    )
                }
                .padding(start = 26.dp, end = 26.dp, top = 34.dp, bottom = 26.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Segmented(listOf("file" to model.t("tabFile"), "text" to model.t("tabText")), s.sourceTab, minWidth = 84.dp) {
                model.dispatch(Ev.sourceTab(it))
                model.haptics.play("selection")
            }
            Spacer(Modifier.padding(11.dp))
            if (s.sourceTab == "file") {
                IconTile(Icons.upload, Lumen.cyan)
                Label(model.t("dropTitle"), size = 22.sp, weight = FontWeight.SemiBold, modifier = Modifier.padding(top = 16.dp))
                Text(s.dropDesc, color = Lumen.muted, fontSize = 15.sp, textAlign = TextAlign.Center, modifier = Modifier.padding(top = 6.dp, bottom = 22.dp))
                LumenButton(model.t("choose"), primary = true) { model.dispatch(Ev.pickFile) }
            } else {
                val shape = RoundedCornerShape(14.dp)
                Box(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = 190.dp, max = 320.dp)
                        .background(Color.Black.copy(alpha = 0.3f), shape)
                        .border(1.dp, if (focused) Lumen.cyan.copy(alpha = 0.55f) else Lumen.line2, shape)
                        .padding(14.dp),
                ) {
                    if (text.isEmpty()) Label(model.t("textPh"), size = 15.sp, color = Lumen.faint)
                    BasicTextField(
                        value = text,
                        onValueChange = { text = it; model.dispatch(Ev.textChanged(it)) },
                        textStyle = TextStyle(color = Lumen.text, fontSize = 15.sp, lineHeight = 22.sp),
                        cursorBrush = SolidColor(Lumen.cyan),
                        modifier = Modifier.fillMaxWidth().heightIn(min = 162.dp).onFocusChanged { focused = it.isFocused },
                    )
                }
                Row(Modifier.fillMaxWidth().padding(top = 8.dp, bottom = 16.dp, start = 2.dp, end = 2.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(s.textMeta, style = mono(12.sp), color = Lumen.faint)
                    Spacer(Modifier.weight(1f))
                    if (text.isEmpty()) {
                        Label(model.t("paste"), size = 12.sp, color = Lumen.cyan, weight = FontWeight.SemiBold, modifier = Modifier.clickable {
                            clipboard.getText()?.text?.let { text = it; model.dispatch(Ev.textChanged(it)) }
                        })
                    } else {
                        Label(model.t("clear"), size = 12.sp, color = Lumen.muted, weight = FontWeight.SemiBold, modifier = Modifier.clickable {
                            text = ""; model.dispatch(Ev.textChanged(""))
                        })
                    }
                }
                LumenButton(model.t("sendText"), primary = true, enabled = s.canSendText) {
                    focus.clearFocus()
                    model.dispatch(Ev.sendText)
                }
            }
            s.error?.let { Label(it, size = 13.sp, color = Lumen.red, modifier = Modifier.padding(top = 14.dp)) }
            Text(s.dropHint, color = Lumen.faint, fontSize = 12.5.sp, textAlign = TextAlign.Center, modifier = Modifier.padding(top = 18.dp))
            Row(
                Modifier.padding(top = 18.dp).clickable { model.dispatch(Ev.navigate("receive")) },
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Label(model.t("goRecv"), size = 13.sp, color = Lumen.muted, weight = FontWeight.Medium)
                androidx.compose.material3.Icon(Icons.arrowRight, null, tint = Lumen.muted, modifier = Modifier.width(16.dp))
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Dock(model: AppModel, s: SendVM) {
    Column(
        Modifier
            .navigationBarsPadding()
            .padding(8.dp)
            .widthIn(max = 720.dp)
            .fillMaxWidth()
            .panel(opaque = true)
            .pointerInput(Unit) { detectTapGestures { model.dispatch(Ev.dockActivity) } }
            .padding(10.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            IconBtn(if (s.playing) Icons.pause else Icons.play, model.t(if (s.playing) "pause" else "play"), enabled = s.phase == "ready") {
                model.dispatch(Ev.togglePlay)
            }
            Row(Modifier.weight(1f), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                StatusDot(live = s.playing && s.phase == "ready", warn = !s.playing)
                Text(s.title, color = Lumen.text, fontSize = 14.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                Text(s.size, style = mono(12.5.sp), color = Lumen.muted, maxLines = 1)
            }
            IconBtn(Icons.sliders, model.t("settings"), active = s.settingsOpen) { model.dispatch(Ev.toggleSettings) }
            IconBtn(Icons.chevronDown, model.t("hideDock")) { model.dispatch(Ev.hideDock) }
            IconBtn(Icons.close, model.t("close")) { model.dispatch(Ev.closeSend) }
        }
        StatsLine(s.stats, Modifier.padding(start = 52.dp, top = 4.dp))
        if (s.settingsOpen) {
            Box(Modifier.padding(vertical = 10.dp).fillMaxWidth().heightIn(min = 1.dp, max = 1.dp).background(Lumen.line))
            Settings(model, s)
        }
    }
}

@Composable
private fun StatsLine(stats: List<StatVM>, modifier: Modifier) {
    BoxWithConstraints(modifier.fillMaxWidth()) {
        // narrow docks drop the "wide" numbers first, as the web dock does under 560 px
        val show = if (maxWidth < 330.dp) stats.filter { !it.wide } else stats
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            for (st in show) {
                Text(
                    buildAnnotatedString {
                        withStyle(SpanStyle(color = Lumen.muted)) { append(st.label + " ") }
                        withStyle(SpanStyle(color = Lumen.text, fontFamily = Lumen.mono, fontWeight = FontWeight.SemiBold)) { append(st.value) }
                    },
                    fontSize = 12.sp, maxLines = 1, softWrap = false,
                )
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Settings(model: AppModel, s: SendVM) {
    var fps by remember { mutableFloatStateOf(s.fps.toFloat()) }
    FlowRow(horizontalArrangement = Arrangement.spacedBy(14.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Field(model.t("density")) {
            Segmented(listOf("low" to model.t("dLow"), "auto" to model.t("dAuto"), "high" to model.t("dHigh"), "max" to model.t("dMax")), s.density) {
                model.dispatch(Ev.density(it))
            }
        }
        Field(model.t("colors")) { Segmented(listOf(1 to "4", 2 to "8"), s.mode) { model.dispatch(Ev.mode(it)) } }
        Field(model.t("ecc")) { Segmented(listOf(0 to model.t("eLow"), 1 to model.t("eMid"), 2 to model.t("eHigh")), s.ecc) { model.dispatch(Ev.ecc(it)) } }
        Field(model.t("speed")) {
            LumenSlider(fps, 4f..60f, 1f, Modifier.width(140.dp)) {
                fps = it
                model.dispatch(Ev.fps(it.roundToInt()))
            }
            Text("${fps.roundToInt()}", style = mono(13.sp, FontWeight.Bold), color = Lumen.text, modifier = Modifier.width(24.dp))
        }
    }
}

@Composable
private fun Field(label: String, content: @Composable () -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Label(label, size = 12.sp, color = Lumen.faint)
        content()
    }
}
