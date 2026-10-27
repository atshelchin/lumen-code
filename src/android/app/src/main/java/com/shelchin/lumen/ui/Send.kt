package com.shelchin.lumen.ui

import android.graphics.Bitmap
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
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
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.layout.displayCutout
import androidx.compose.foundation.layout.union
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.shelchin.lumen.BuildConfig
import com.shelchin.lumen.core.AppModel
import com.shelchin.lumen.core.Ev
import com.shelchin.lumen.core.GeometryVM
import com.shelchin.lumen.core.SendVM
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

/** Insets the code must stay clear of: system bars and the cutout (not the keyboard). */
private val safeArea: WindowInsets
    @Composable get() = WindowInsets.systemBars.union(WindowInsets.displayCutout)

/**
 * The sender (the web's send page): pick a file or type text, then the code. While a code is
 * shown nothing overlaps it: the code takes the area above a permanent bar (and beside or above
 * the settings, when open), and that area, not the screen, is what the core fits the grid into.
 */
@Composable
fun SendScreen(model: AppModel, v: ViewModel) {
    val s = v.send
    val coding = s.phase != "pick"
    Box(Modifier.fillMaxSize().then(if (coding) Modifier.background(Lumen.ink) else Modifier)) {
        // backgrounds run edge to edge; contents keep clear of the bars and the cutout
        BoxWithConstraints(Modifier.fillMaxSize()) {
            val landscape = maxWidth > maxHeight
            val settings = coding && s.settingsOpen
            val sideWidth = minOf(360.dp, maxWidth * 0.46f)
            val panelMax = maxHeight * 0.5f
            // the same skeleton on the pick screen, so the first code is laid out for its real area
            Column(Modifier.fillMaxSize()) {
                Row(Modifier.weight(1f).fillMaxWidth()) {
                    // beside the settings, the panel takes the end inset
                    val sides = if (settings && landscape) WindowInsetsSides.Start + WindowInsetsSides.Top else WindowInsetsSides.Horizontal + WindowInsetsSides.Top
                    CodeArea(model, s, Modifier.weight(1f).fillMaxHeight().windowInsetsPadding(safeArea.only(sides)))
                    if (settings && landscape) SettingsPanel(model, s, side = true, Modifier.width(sideWidth).fillMaxHeight())
                }
                if (settings && !landscape) SettingsPanel(model, s, side = false, Modifier.fillMaxWidth().heightIn(max = panelMax))
                SendBar(model, s, coding)
            }
        }
        if (!coding) {
            Box(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) { PickCard(model, s) }
            Box(Modifier.windowInsetsPadding(WindowInsets.safeDrawing).padding(horizontal = 10.dp, vertical = 8.dp)) {
                HomeButton(model.t("home")) { model.dispatch(Ev.navigate("home")) }
            }
        } else if (s.flashAsk && s.phase == "ready") {
            FlashCard(model, s)
        }
    }
}

/** Where the code goes: its size is the viewport the core fits the grid (quiet zone included) into. */
@Composable
private fun CodeArea(model: AppModel, s: SendVM, modifier: Modifier) {
    val context = LocalContext.current
    BoxWithConstraints(modifier) {
        val density = LocalDensity.current.density
        val w = constraints.maxWidth
        val h = constraints.maxHeight
        LaunchedEffect(w, h) {
            val dm = context.resources.displayMetrics
            // physical pixel size; some devices report a nonsense xdpi, fall back to the density bucket
            val dpi = if (abs(dm.xdpi - dm.densityDpi) / dm.densityDpi < 0.3f) dm.xdpi else dm.densityDpi.toFloat()
            model.dispatch(Ev.viewport(w.toDouble(), h.toDouble(), density.toDouble(), 25.4 / dpi))
        }
        if (s.phase != "pick") s.geometry?.let { CodeView(model.core, it, s.frameRev, s.playing, s.fps) }
    }
}

/**
 * The bar under the code: play/pause, what is being sent, settings, close. One row, always
 * there. On the pick screen it only holds its place (the pick card covers the screen); under
 * the photosensitivity warning it is empty, so the warning stands alone.
 */
@Composable
private fun SendBar(model: AppModel, s: SendVM, coding: Boolean) {
    Column(Modifier.fillMaxWidth().then(if (coding) Modifier.background(Lumen.bg2) else Modifier)) {
        if (coding) Box(Modifier.fillMaxWidth().height(1.dp).background(Lumen.line))
        Row(
            Modifier
                .fillMaxWidth()
                .windowInsetsPadding(safeArea.only(WindowInsetsSides.Bottom + WindowInsetsSides.Horizontal))
                .height(64.dp)
                .padding(horizontal = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            if (!coding || s.flashAsk) return@Row
            val ready = s.phase == "ready"
            PlayButton(s.playing, model.t(if (s.playing) "pause" else "play"), enabled = ready) { model.dispatch(Ev.togglePlay) }
            Column(Modifier.weight(1f).padding(horizontal = 6.dp)) {
                TitleLine(s.title)
                Text(
                    s.subtitle, color = if (ready && !s.playing) Lumen.amber else Lumen.muted, fontSize = 12.5.sp,
                    maxLines = 1, overflow = TextOverflow.Ellipsis,
                )
            }
            BarButton(Icons.sliders, model.t("settings"), selected = s.settingsOpen) { model.dispatch(Ev.toggleSettings) }
            BarButton(Icons.close, model.t("close")) { model.dispatch(Ev.closeSend) }
        }
    }
}

/** A long file name loses its middle, never its end: "holiday-ph…inal.zip". */
@Composable
private fun TitleLine(title: String) {
    val dot = title.lastIndexOf('.')
    val ext = if (dot > 0 && title.length - dot <= 8) title.substring(dot) else ""
    val stem = title.substring(0, title.length - ext.length)
    val keep = minOf(stem.length, 4)
    val head = stem.substring(0, stem.length - keep)
    val tail = stem.substring(stem.length - keep) + ext
    Row(Modifier.semantics(mergeDescendants = true) { contentDescription = title }) {
        val style = TextStyle(color = Lumen.text, fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
        // on one baseline: a CJK head and a Latin tail come from fonts of different heights
        if (head.isNotEmpty()) Text(head, style = style, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false).alignByBaseline())
        Text(tail, style = style, maxLines = 1, softWrap = false, modifier = Modifier.alignByBaseline())
    }
}

@Composable
private fun PlayButton(playing: Boolean, description: String, enabled: Boolean, onClick: () -> Unit) {
    Box(
        Modifier
            .size(48.dp)
            .alpha(if (enabled) 1f else 0.4f)
            .clip(CircleShape)
            .background(Lumen.paper)
            .clickable(enabled = enabled, role = Role.Button, onClickLabel = null, onClick = onClick)
            .semantics { contentDescription = description },
        contentAlignment = Alignment.Center,
    ) {
        androidx.compose.material3.Icon(if (playing) Icons.pause else Icons.play, contentDescription = null, tint = Lumen.ink, modifier = Modifier.size(22.dp))
    }
}

@Composable
private fun BarButton(icon: ImageVector, description: String, selected: Boolean? = null, onClick: () -> Unit) {
    val on = selected == true
    Box(
        Modifier
            .size(48.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(if (on) Color.White.copy(alpha = 0.12f) else Color.Transparent)
            .clickable(role = Role.Button, onClick = onClick)
            .semantics {
                contentDescription = description
                if (selected != null) this.selected = on
            },
        contentAlignment = Alignment.Center,
    ) {
        androidx.compose.material3.Icon(icon, contentDescription = null, tint = if (on) Lumen.text else Lumen.muted, modifier = Modifier.size(22.dp))
    }
}

/**
 * The photosensitivity warning over the paused code, before the first code plays. The code
 * waits until it is accepted; closing returns to the pick screen.
 */
@Composable
private fun FlashCard(model: AppModel, s: SendVM) {
    val title = model.t("flashTitle")
    Box(
        Modifier
            .fillMaxSize()
            .background(Color.Black.copy(alpha = 0.6f))
            // touches stay on the card, never reach the code or the bar underneath
            .pointerInput(Unit) { detectTapGestures { } }
            .windowInsetsPadding(WindowInsets.safeDrawing),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            Modifier
                .verticalScroll(rememberScrollState())
                .padding(16.dp)
                .widthIn(max = 440.dp)
                .fillMaxWidth()
                .panel(opaque = true, radius = 20.dp)
                .border(1.dp, Lumen.amber.copy(alpha = 0.25f), RoundedCornerShape(20.dp))
                .semantics { paneTitle = title }
                .padding(horizontal = 22.dp, vertical = 24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            IconTile(Icons.warn, Lumen.amber, size = 52.dp)
            Text(
                title, color = Lumen.text, fontSize = 20.sp, fontWeight = FontWeight.SemiBold, textAlign = TextAlign.Center,
                modifier = Modifier.padding(top = 14.dp).semantics { heading() },
            )
            Text(
                s.flashWarn, color = Lumen.warnText, fontSize = 15.sp, lineHeight = 22.sp, textAlign = TextAlign.Center,
                modifier = Modifier.padding(top = 10.dp),
            )
            LumenButton(model.t("flashPlay"), primary = true, lines = 3, modifier = Modifier.padding(top = 22.dp).fillMaxWidth()) {
                model.dispatch(Ev.acceptFlash)
            }
            LumenButton(model.t("close"), lines = 2, modifier = Modifier.padding(top = 10.dp).fillMaxWidth()) {
                model.dispatch(Ev.closeSend)
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
private fun CodeView(core: LumenCore, g: GeometryVM, frameRev: Long, playing: Boolean, fps: Int) {
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
    Canvas(Modifier.fillMaxSize()) {
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
    // the core holds the text: what the field shows is what "Send text" sends
    val text = s.text
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
                        onValueChange = { model.dispatchNow(Ev.textChanged(it)) },
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
                            clipboard.getText()?.text?.let { model.dispatchNow(Ev.textChanged(it)) }
                        })
                    } else {
                        Label(model.t("clear"), size = 12.sp, color = Lumen.muted, weight = FontWeight.SemiBold, modifier = Modifier.clickable {
                            model.dispatchNow(Ev.textChanged(""))
                        })
                    }
                }
                LumenButton(model.t("sendText"), primary = true, enabled = s.canSendText) {
                    focus.clearFocus()
                    model.dispatch(Ev.sendText)
                }
            }
            (model.sendNote ?: s.error)?.let {
                Text(
                    it, color = Lumen.red, fontSize = 13.sp, textAlign = TextAlign.Center,
                    modifier = Modifier.padding(top = 14.dp).semantics { liveRegion = LiveRegionMode.Polite },
                )
            }
            Text(s.dropHint, color = Lumen.faint, fontSize = 12.5.sp, textAlign = TextAlign.Center, modifier = Modifier.padding(top = 18.dp))
            // photosensitivity warning, before any code plays
            Row(
                Modifier.padding(top = 14.dp).widthIn(max = 460.dp)
                    .background(Lumen.amber.copy(alpha = 0.06f), RoundedCornerShape(12.dp))
                    .border(1.dp, Lumen.amber.copy(alpha = 0.22f), RoundedCornerShape(12.dp))
                    .padding(horizontal = 12.dp, vertical = 10.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                androidx.compose.material3.Icon(Icons.warn, null, tint = Lumen.amber, modifier = Modifier.padding(top = 1.dp).width(16.dp))
                Text(s.flashWarn, color = Lumen.warnText, fontSize = 12.5.sp, lineHeight = 18.sp)
            }
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

/**
 * The settings, attached above the bar (beside the code in landscape). The code shrinks to the
 * space left; the core regrids and the stream goes on.
 */
@Composable
private fun SettingsPanel(model: AppModel, s: SendVM, side: Boolean, modifier: Modifier) {
    var fps by remember { mutableFloatStateOf(s.fps.toFloat()) }
    val title = model.t("settings")
    Row(modifier.background(Lumen.bg2).semantics { paneTitle = title }) {
        if (side) Box(Modifier.fillMaxHeight().width(1.dp).background(Lumen.line))
        Column(Modifier.fillMaxWidth()) {
            if (!side) Box(Modifier.fillMaxWidth().height(1.dp).background(Lumen.line))
            Column(
                Modifier
                    .windowInsetsPadding(safeArea.only(if (side) WindowInsetsSides.End + WindowInsetsSides.Top else WindowInsetsSides.Horizontal))
                    .verticalScroll(rememberScrollState())
                    .padding(start = if (side) 14.dp else 12.dp, end = 12.dp, top = 10.dp, bottom = 8.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Field(model.t("density")) {
                    Segmented(
                        listOf("auto" to model.t("dAuto"), "low" to model.t("dLow"), "high" to model.t("dHigh"), "max" to model.t("dMax")),
                        s.density, modifier = Modifier.weight(1f), fill = true, itemHeight = 40.dp,
                    ) { model.dispatch(Ev.density(it)) }
                }
                Field(model.t("colors")) {
                    Segmented(listOf(1 to "4", 2 to "8"), s.mode, modifier = Modifier.weight(1f), fill = true, itemHeight = 40.dp) { model.dispatch(Ev.mode(it)) }
                }
                Field(model.t("ecc")) {
                    Segmented(
                        listOf(0 to model.t("eLow"), 1 to model.t("eMid"), 2 to model.t("eHigh")),
                        s.ecc, modifier = Modifier.weight(1f), fill = true, itemHeight = 40.dp,
                    ) { model.dispatch(Ev.ecc(it)) }
                }
                Field(model.t("speed")) {
                    LumenSlider(fps, 4f..60f, 1f, Modifier.weight(1f)) {
                        fps = it
                        model.dispatch(Ev.fps(it.roundToInt()))
                    }
                    Text("${fps.roundToInt()}", style = mono(13.sp, FontWeight.Bold), color = Lumen.text, textAlign = TextAlign.End, modifier = Modifier.width(28.dp))
                }
                // two lines, always: the panel's height must not follow the numbers (it sets the code's area)
                Text(
                    buildAnnotatedString {
                        withStyle(SpanStyle(color = Lumen.muted, fontWeight = FontWeight.SemiBold)) { append(model.t("details")) }
                        append("  ")
                        append(s.details)
                    },
                    color = Lumen.faint, fontSize = 11.5.sp, lineHeight = 17.sp, minLines = 2, maxLines = 2, overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(top = 2.dp),
                )
            }
        }
    }
}

@Composable
private fun Field(label: String, content: @Composable RowScope.() -> Unit) {
    Row(Modifier.fillMaxWidth().heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Text(label, color = Lumen.muted, fontSize = 13.sp, lineHeight = 17.sp, maxLines = 2, modifier = Modifier.width(88.dp))
        content()
    }
}
