package com.shelchin.lumen.ui

import android.view.ViewGroup
import android.content.Context
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.heightIn
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.collapse
import androidx.compose.ui.semantics.expand
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import com.shelchin.lumen.core.AppModel
import com.shelchin.lumen.core.Ev
import com.shelchin.lumen.core.OverlayVM
import com.shelchin.lumen.core.RecvVM
import com.shelchin.lumen.core.ViewModel

/** The receiver (the web's recv.html): start screen, live camera with the dashboard, result. */
@Composable
fun ReceiveScreen(model: AppModel, v: ViewModel) {
    val r = v.recv
    Box(Modifier.fillMaxSize().background(Color.Black)) {
        if (r.cameraOn || r.cameraStarting) {
            val zoom by rememberUpdatedState(r.zoom?.level ?: 1f)
            Box(
                Modifier
                    .fillMaxSize()
                    .pointerInput(Unit) {
                        detectTransformGestures { _, _, scale, _ -> if (scale != 1f) model.dispatch(Ev.zoom(zoom * scale)) }
                    },
            ) {
                AndroidView(
                    factory = { model.camera.previewView.also { (it.parent as? ViewGroup)?.removeView(it) } },
                    modifier = Modifier.fillMaxSize(),
                )
                OutlineLayer(model)
            }
        }
        if (r.phase == "scanning") ScanningLayer(model, r)
        if (r.phase == "start") StartPane(model, r)
        if (r.phase == "done") r.done?.let { DoneScreen(model, it) }
        if (r.phase != "done") {
            Row(
                Modifier.windowInsetsPadding(WindowInsets.safeDrawing).padding(horizontal = 10.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                HomeButton(model.t("home")) { model.dispatch(Ev.navigate("home")) }
                if (r.phase == "scanning") Chip {
                    StatusDot(r.live)
                    Label(r.status, size = 13.sp, weight = FontWeight.SemiBold, lines = 1)
                }
            }
        }
    }
}

/** The detected code outline, mapped from camera-frame pixels onto the fill-centred preview. */
/** The outline, in its own scope: it changes 20 times a second, the rest of the screen does not. */
@Composable
private fun OutlineLayer(model: AppModel) {
    val o = model.overlay ?: return
    Outline(o, model.camera.mirrored)
}

@Composable
private fun Outline(o: OverlayVM, mirrored: Boolean) {
    val color = when (o.tone) {
        "good" -> Lumen.green
        "fair" -> Lumen.amber
        else -> Lumen.red
    }
    Canvas(Modifier.fillMaxSize()) {
        if (o.points.size != 4 || o.frameW <= 0) return@Canvas
        // the preview and the analysed frames are the same 16:9 stream, shown upright and
        // scaled to fill the view (PreviewView FILL_CENTER)
        val rot = ((o.rotation % 360) + 360) % 360
        val (dw, dh) = if (rot % 180 == 90) o.frameH.toFloat() to o.frameW.toFloat() else o.frameW.toFloat() to o.frameH.toFloat()
        val k = maxOf(size.width / dw, size.height / dh)
        val ox = (size.width - dw * k) / 2
        val oy = (size.height - dh * k) / 2
        fun map(p: List<Float>): Offset {
            val (x, y) = p[0] to p[1]
            val (rx, ry) = when (rot) {
                90 -> (o.frameH - y) to x
                180 -> (o.frameW - x) to (o.frameH - y)
                270 -> y to (o.frameW - x)
                else -> x to y
            }
            val px = ox + rx * k
            return Offset(if (mirrored) size.width - px else px, oy + ry * k)
        }
        val path = Path().apply {
            o.points.forEachIndexed { i, p -> map(p).let { if (i == 0) moveTo(it.x, it.y) else lineTo(it.x, it.y) } }
            close()
        }
        drawPath(path, color.copy(alpha = 0.35f), style = Stroke(width = 9.dp.toPx(), join = StrokeJoin.Round))
        drawPath(path, color, style = Stroke(width = 3.dp.toPx(), join = StrokeJoin.Round))
    }
}

/**
 * While scanning: framing guides over the image until a code is found, the camera tools in a
 * row just above the panel (never over the middle of the image), and the panel at the bottom
 * (bottom right in landscape).
 */
@Composable
private fun ScanningLayer(model: AppModel, r: RecvVM) {
    val context = LocalContext.current
    val prefs = remember { context.getSharedPreferences("lumen", Context.MODE_PRIVATE) }
    var details by remember { mutableStateOf(prefs.getBoolean(DETAILS_KEY, false)) }
    BoxWithConstraints(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
        val landscape = maxWidth > maxHeight
        val dockWidth = if (landscape) minOf(340.dp, maxWidth * 0.44f) else minOf(560.dp, maxWidth - 20.dp)
        var dockHeight by remember { mutableIntStateOf(0) }
        val dockH = with(LocalDensity.current) { dockHeight.toDp() }
        if (!r.live) {
            Guides(
                Modifier.fillMaxSize().padding(
                    top = 62.dp,
                    bottom = if (landscape) 18.dp else dockH + 6.dp + 18.dp,
                    start = 22.dp,
                    end = if (landscape) dockWidth + 10.dp + 22.dp else 22.dp,
                ),
            )
        }
        Column(
            Modifier
                .align(if (landscape) Alignment.BottomEnd else Alignment.BottomCenter)
                .padding(horizontal = 10.dp, vertical = 6.dp)
                .width(dockWidth)
                .onSizeChanged { dockHeight = it.height },
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            if (r.zoom != null || r.canSwitch) {
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End), verticalAlignment = Alignment.CenterVertically) {
                    r.zoom?.let { z ->
                        Box(
                            Modifier
                                .clip(CircleShape)
                                .background(Color(0xDB0C1017))
                                .border(1.dp, if (z.active) Lumen.cyan else Lumen.line2, CircleShape)
                                .clickable { model.dispatch(Ev.resetZoom) }
                                .defaultMinSize(minWidth = 48.dp, minHeight = 36.dp)
                                .padding(horizontal = 12.dp),
                            contentAlignment = Alignment.Center,
                        ) { Text(z.label, style = mono(13.sp, FontWeight.SemiBold), color = Lumen.text) }
                    }
                    if (r.canSwitch) IconBtn(Icons.switchCamera, model.t("switchCam"), background = Color(0xDB0C1017), size = 48.dp) { model.dispatch(Ev.switchCamera) }
                }
            }
            Hud(model, r, details) {
                details = !details
                prefs.edit().putBoolean(DETAILS_KEY, details).apply()
            }
        }
    }
}

private const val DETAILS_KEY = "recvDetails"

/** Four corner brackets: where to put the code. */
@Composable
private fun Guides(modifier: Modifier) {
    Canvas(modifier) {
        val arm = 30.dp.toPx()
        val rad = 10.dp.toPx()
        val sw = 3.dp.toPx()
        val h = sw / 2
        if (size.width < arm * 2 || size.height < arm * 2) return@Canvas
        val (w, ht) = size.width to size.height
        // each corner: an arm along each edge and a rounded turn
        fun corner(x: Float, y: Float, dx: Float, dy: Float) = Path().apply {
            moveTo(x, y + dy * arm)
            lineTo(x, y + dy * rad)
            quadraticTo(x, y, x + dx * rad, y)
            lineTo(x + dx * arm, y)
        }
        val corners = listOf(corner(h, h, 1f, 1f), corner(w - h, h, -1f, 1f), corner(h, ht - h, 1f, -1f), corner(w - h, ht - h, -1f, -1f))
        for (p in corners) {
            drawPath(p, Color.Black.copy(alpha = 0.35f), style = Stroke(width = sw + 3.dp.toPx(), cap = StrokeCap.Round))
            drawPath(p, Color.White.copy(alpha = 0.75f), style = Stroke(width = sw, cap = StrokeCap.Round))
        }
    }
}

@Composable
private fun Hud(model: AppModel, r: RecvVM, details: Boolean, toggleDetails: () -> Unit) {
    // no animation: progress already moves 20 times a second, and an animation would redraw
    // the panel on every display frame while receiving
    val progress = r.progress
    Column(Modifier.fillMaxWidth().panel(opaque = true).padding(horizontal = 16.dp, vertical = 14.dp)) {
        Row(verticalAlignment = Alignment.Bottom) {
            Text(r.name, color = Lumen.text, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
            Spacer(Modifier.width(12.dp))
            Text(r.pct, style = mono(22.sp, FontWeight.Bold), color = Lumen.text)
        }
        Box(Modifier.padding(top = 10.dp).fillMaxWidth().height(8.dp).clip(CircleShape).background(Color.White.copy(alpha = 0.08f))) {
            Box(Modifier.fillMaxHeight().fillMaxWidth(progress.coerceIn(0f, 1f)).clip(CircleShape).background(Lumen.spectrum))
        }
        // the row keeps its height while the line is empty: nothing jumps when data arrives
        Row(Modifier.padding(top = 6.dp).fillMaxWidth().heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(r.line, color = Lumen.text, fontSize = 13.5.sp, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
            Spacer(Modifier.width(10.dp))
            DetailsToggle(model.t("details"), details, toggleDetails)
        }
        Label(r.tip, size = 13.sp, color = if (r.tipOk) Lumen.muted else Lumen.amber, lines = 2)
        if (details) {
            Box(Modifier.padding(top = 10.dp).fillMaxWidth().height(1.dp).background(Lumen.line))
            FlowRow(Modifier.padding(top = 10.dp), horizontalArrangement = Arrangement.spacedBy(18.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                for (m in r.metrics) {
                    Text(
                        buildAnnotatedString {
                            withStyle(SpanStyle(color = Lumen.muted)) { append(m.label + " ") }
                            withStyle(SpanStyle(color = Lumen.text, fontFamily = Lumen.mono, fontWeight = FontWeight.SemiBold)) { append(m.value) }
                        },
                        fontSize = 12.5.sp, maxLines = 1, softWrap = false,
                    )
                }
            }
            Text(r.diag, style = mono(11.sp), color = Lumen.faint, lineHeight = 16.sp, maxLines = 3, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 8.dp))
        }
    }
}

/** "Details ⌄": folds the numbers in and out (remembered across launches). */
@Composable
private fun DetailsToggle(label: String, open: Boolean, toggle: () -> Unit) {
    val rot by animateFloatAsState(if (open) 180f else 0f, label = "chevron")
    Row(
        Modifier
            .minimumInteractiveComponentSize()
            .clip(CircleShape)
            .border(1.dp, Lumen.line2, CircleShape)
            .background(Color.White.copy(alpha = 0.04f))
            .clickable(role = Role.Button, onClick = toggle)
            .semantics {
                if (open) collapse { toggle(); true } else expand { toggle(); true }
            }
            .defaultMinSize(minHeight = 36.dp)
            .padding(start = 12.dp, end = 9.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text(label, color = Lumen.muted, fontSize = 13.sp, maxLines = 1)
        Icon(Icons.chevronDown, contentDescription = null, tint = Lumen.muted, modifier = Modifier.size(16.dp).rotate(rot))
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun StartPane(model: AppModel, r: RecvVM) {
    Box(
        Modifier
            .fillMaxSize()
            .background(Lumen.bg)
            .background(Brush.radialGradient(listOf(Lumen.pink.copy(alpha = 0.08f), Color.Transparent), center = Offset(540f, 600f), radius = 1100f)),
    ) {
        Column(
            Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing).verticalScroll(rememberScrollState()).padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
        ) {
            Column(Modifier.widthIn(max = 420.dp).padding(top = 40.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                IconTile(Icons.viewfinder, Lumen.pink)
                Label(model.t("recvStartTitle"), size = 24.sp, weight = FontWeight.Bold, modifier = Modifier.padding(top = 18.dp, bottom = 8.dp))
                Text(model.t("recvStartDesc"), color = Lumen.muted, fontSize = 15.sp, textAlign = TextAlign.Center, modifier = Modifier.padding(bottom = 22.dp))
                // permission denied: asking again does nothing, so Settings is the way forward
                val denied = r.canOpenSettings
                if (denied) r.error?.let {
                    Text(it, color = Lumen.red, fontSize = 14.sp, textAlign = TextAlign.Center, modifier = Modifier.padding(bottom = 16.dp))
                }
                FlowRow(horizontalArrangement = Arrangement.spacedBy(10.dp, Alignment.CenterHorizontally), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    if (denied) LumenButton(model.t("openSettings"), primary = true) { model.dispatch(Ev.openSettings) }
                    LumenButton(model.t("openCam"), primary = !denied, enabled = !r.cameraStarting, leading = if (r.cameraStarting) {
                        { CircularProgressIndicator(color = if (denied) Lumen.text else Lumen.ink, strokeWidth = 2.dp, modifier = Modifier.size(16.dp)) }
                    } else null) { model.dispatch(Ev.openCamera) }
                    LumenButton(model.t("fromImage")) { model.dispatch(Ev.pickImage) }
                }
                if (!denied) r.error?.let {
                    Text(it, color = Lumen.red, fontSize = 13.sp, textAlign = TextAlign.Center, modifier = Modifier.padding(top = 14.dp))
                }
                r.note?.let { Text(it, color = Lumen.amber, fontSize = 13.sp, textAlign = TextAlign.Center, modifier = Modifier.padding(top = 14.dp)) }
                Row(
                    Modifier.padding(top = 26.dp).clickable { model.dispatch(Ev.navigate("send")) },
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    Label(model.t("goSend"), size = 13.sp, color = Lumen.muted, weight = FontWeight.Medium)
                    Icon(Icons.arrowRight, null, tint = Lumen.muted, modifier = Modifier.size(16.dp))
                }
            }
        }
    }
}
