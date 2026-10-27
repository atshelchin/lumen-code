package com.shelchin.lumen.ui

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.setProgress
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextAlign
import kotlin.math.roundToInt
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** Design tokens of the web demo (src/web/css/app.css), one to one. */
object Lumen {
    val bg = Color(0xFF07090D)
    val bg2 = Color(0xFF0C1017)
    val ink = Color(0xFF0A0D14)
    val panel = Color(0xC711151E)
    /** Panels over live video or a playing code: no blur (heat), more opaque. */
    val panelOpaque = Color(0xF211151E)
    val line = Color(0x14FFFFFF)
    val line2 = Color(0x24FFFFFF)
    val text = Color(0xFFE9EDF4)
    val muted = Color(0xFF8B94A7)
    val faint = Color(0xFF5B6476)
    val cyan = Color(0xFF22D8FF)
    val pink = Color(0xFFFF3D9A)
    val amber = Color(0xFFFFD60A)
    val warnText = Color(0xFFE8D48A)
    val green = Color(0xFF2BDC74)
    val red = Color(0xFFFF5468)
    val paper = Color(0xFFF2F4F8)
    val markDark = Color(0xFF1B2130)
    val spectrum = Brush.horizontalGradient(listOf(cyan, pink, amber))
    val hero = Brush.horizontalGradient(0f to cyan, 0.55f to pink, 1f to amber)
    val mono = FontFamily.Monospace
}

fun mono(size: TextUnit, weight: FontWeight = FontWeight.Normal) = TextStyle(fontFamily = Lumen.mono, fontSize = size, fontWeight = weight)

@Composable
fun BrandMark(cell: Dp = 7.dp) {
    val colors = listOf(Lumen.cyan, Lumen.markDark, Lumen.pink, Lumen.markDark, Lumen.paper, Lumen.amber, Lumen.markDark, Lumen.cyan, Lumen.green)
    val gap = cell * 3 / 7
    Column(
        Modifier
            .clip(RoundedCornerShape(cell * 9 / 7))
            .background(Lumen.ink)
            .border(1.dp, Lumen.line2, RoundedCornerShape(cell * 9 / 7))
            .padding(cell * 6 / 7),
        verticalArrangement = Arrangement.spacedBy(gap),
    ) {
        for (r in 0 until 3) Row(horizontalArrangement = Arrangement.spacedBy(gap)) {
            for (c in 0 until 3) Box(Modifier.size(cell).clip(RoundedCornerShape(cell * 2 / 7)).background(colors[r * 3 + c]))
        }
    }
}

@Composable
fun LumenButton(
    label: String,
    primary: Boolean = false,
    enabled: Boolean = true,
    modifier: Modifier = Modifier,
    leading: (@Composable () -> Unit)? = null,
    /** More than one only where a long label must stay whole at large font sizes. */
    lines: Int = 1,
    onClick: () -> Unit,
) {
    val src = remember { MutableInteractionSource() }
    val pressed by src.collectIsPressedAsState()
    val scale by animateFloatAsState(if (pressed) 0.98f else 1f, label = "press")
    val shape = RoundedCornerShape(12.dp)
    Row(
        modifier
            .scale(scale)
            .alpha(if (enabled) 1f else 0.4f)
            .clip(shape)
            .background(if (primary) (if (pressed) Color.White else Lumen.paper) else Color.White.copy(alpha = if (pressed) 0.1f else 0.04f))
            .then(if (primary) Modifier else Modifier.border(1.dp, Lumen.line2, shape))
            .clickable(interactionSource = src, indication = null, enabled = enabled, role = Role.Button, onClick = onClick)
            .defaultMinSize(minHeight = 44.dp)
            .padding(horizontal = 18.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
    ) {
        leading?.invoke()
        Text(label, color = if (primary) Lumen.ink else Lumen.text, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, maxLines = lines, textAlign = TextAlign.Center)
    }
}

@Composable
fun IconBtn(icon: ImageVector, description: String, active: Boolean = false, background: Color = Color.White.copy(alpha = 0.04f), enabled: Boolean = true, size: Dp = 44.dp, onClick: () -> Unit) {
    val src = remember { MutableInteractionSource() }
    val pressed by src.collectIsPressedAsState()
    val shape = RoundedCornerShape(12.dp)
    Box(
        Modifier
            .size(size)
            .scale(if (pressed) 0.96f else 1f)
            .alpha(if (enabled) 1f else 0.4f)
            .clip(shape)
            .background(if (active) Color.White.copy(alpha = 0.12f) else if (pressed) Color.White.copy(alpha = 0.1f) else background)
            .border(1.dp, Lumen.line2, shape)
            .clickable(interactionSource = src, indication = null, enabled = enabled, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, contentDescription = description, tint = Lumen.text, modifier = Modifier.size(20.dp))
    }
}

/** The web's `.tbtn`: small tool buttons in the reader. */
@Composable
fun ToolBtn(pressed: Boolean = false, description: String, onClick: () -> Unit, content: @Composable () -> Unit) {
    val shape = RoundedCornerShape(9.dp)
    Box(
        Modifier
            .defaultMinSize(minWidth = 36.dp, minHeight = 34.dp)
            .clip(shape)
            .background(if (pressed) Color.White.copy(alpha = 0.12f) else Color.White.copy(alpha = 0.04f))
            .border(1.dp, if (pressed) Lumen.line2 else Lumen.line, shape)
            .clickable(onClickLabel = description, onClick = onClick)
            .padding(horizontal = 8.dp),
        contentAlignment = Alignment.Center,
    ) { content() }
}

@Composable
fun <T> Segmented(
    items: List<Pair<T, String>>,
    selected: T,
    minWidth: Dp = 0.dp,
    modifier: Modifier = Modifier,
    /** Every item takes an equal share of the width the segmented control is given. */
    fill: Boolean = false,
    itemHeight: Dp = 32.dp,
    pick: (T) -> Unit,
) {
    val outer = RoundedCornerShape(11.dp)
    Row(
        modifier
            .clip(outer)
            .background(Color.White.copy(alpha = 0.05f))
            .border(1.dp, Lumen.line, outer)
            .selectableGroup()
            .padding(3.dp),
        horizontalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        for ((value, label) in items) {
            val on = value == selected
            Box(
                Modifier
                    .then(if (fill) Modifier.weight(1f) else Modifier)
                    .defaultMinSize(minWidth = minWidth, minHeight = itemHeight)
                    .clip(RoundedCornerShape(8.dp))
                    .background(if (on) Color.White.copy(alpha = 0.12f) else Color.Transparent)
                    .selectable(selected = on, role = Role.RadioButton) { if (!on) pick(value) }
                    .padding(horizontal = 11.dp),
                contentAlignment = Alignment.Center,
            ) {
                Text(label, color = if (on) Lumen.text else Lumen.muted, fontSize = 13.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

fun Modifier.panel(opaque: Boolean = false, radius: Dp = 16.dp): Modifier {
    val shape = RoundedCornerShape(radius)
    // An opaque panel sits over live video or a playing code and changes many times a second:
    // a soft shadow there costs a shadow pass on every redraw and is invisible anyway.
    val base = if (opaque) this else this.shadow(20.dp, shape, ambientColor = Color.Black.copy(alpha = 0.45f), spotColor = Color.Black.copy(alpha = 0.45f))
    return base
        .clip(shape)
        .background(if (opaque) Lumen.panelOpaque else Lumen.panel)
        .border(BorderStroke(1.dp, Lumen.line), shape)
}

/** The web's range input: a cyan track and a round white thumb. */
@Composable
fun LumenSlider(value: Float, range: ClosedFloatingPointRange<Float>, step: Float, modifier: Modifier = Modifier, onChange: (Float) -> Unit) {
    val current by rememberUpdatedState(value)
    val change by rememberUpdatedState(onChange)
    fun pick(x: Float, width: Float) {
        val f = (x / width).coerceIn(0f, 1f)
        val raw = range.start + f * (range.endInclusive - range.start)
        val v = (Math.round(raw / step) * step).coerceIn(range.start, range.endInclusive)
        if (v != current) change(v)
    }
    val steps = (((range.endInclusive - range.start) / step).roundToInt() - 1).coerceAtLeast(0)
    Canvas(
        modifier
            .height(32.dp)
            // TalkBack: read as a value in a range, adjusted with volume keys / swipes
            .semantics {
                progressBarRangeInfo = ProgressBarRangeInfo(value, range, steps)
                stateDescription = if (step >= 1f) value.roundToInt().toString() else value.toString()
                setProgress { target ->
                    val v = (Math.round(target / step) * step).coerceIn(range.start, range.endInclusive)
                    if (v != current) change(v)
                    true
                }
            }
            .pointerInput(range) { detectTapGestures { pick(it.x, size.width.toFloat()) } }
            .pointerInput(range) { detectHorizontalDragGestures { ch, _ -> pick(ch.position.x, size.width.toFloat()) } },
    ) {
        val f = ((value - range.start) / (range.endInclusive - range.start)).coerceIn(0f, 1f)
        val r = 9.dp.toPx()
        val y = size.height / 2
        val x = r + f * (size.width - 2 * r)
        val track = 4.dp.toPx()
        drawLine(Lumen.line2, Offset(r, y), Offset(size.width - r, y), strokeWidth = track, cap = StrokeCap.Round)
        drawLine(Lumen.cyan, Offset(r, y), Offset(x, y), strokeWidth = track, cap = StrokeCap.Round)
        drawCircle(Color.Black.copy(alpha = 0.3f), r + 1.dp.toPx(), Offset(x, y + 1.dp.toPx()))
        drawCircle(Color.White, r, Offset(x, y))
    }
}

@Composable
fun StatusDot(live: Boolean, warn: Boolean = false) {
    Box(contentAlignment = Alignment.Center, modifier = Modifier.size(16.dp)) {
        if (live) Box(Modifier.size(16.dp).clip(CircleShape).background(Lumen.green.copy(alpha = 0.15f)))
        Box(Modifier.size(8.dp).clip(CircleShape).background(if (live) Lumen.green else if (warn) Lumen.amber else Lumen.faint))
    }
}

/** Rounded tinted square holding an icon (the web's `.drop-art` / `.choice .ico`). */
@Composable
fun IconTile(icon: ImageVector, tint: Color = Lumen.cyan, size: Dp = 64.dp) {
    Box(
        Modifier.size(size).clip(RoundedCornerShape(size * 0.28f)).background(tint.copy(alpha = 0.11f)),
        contentAlignment = Alignment.Center,
    ) { Icon(icon, contentDescription = null, tint = tint, modifier = Modifier.size(size * 0.46f)) }
}

/** "‹ mark" — back to the home screen. */
@Composable
fun HomeButton(description: String, onClick: () -> Unit) {
    Row(
        Modifier
            .clip(RoundedCornerShape(10.dp))
            .clickable(role = Role.Button, onClick = onClick)
            .semantics(mergeDescendants = true) { contentDescription = description }
            .defaultMinSize(minWidth = 48.dp, minHeight = 48.dp)
            .padding(4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Icon(Icons.chevronLeft, contentDescription = null, tint = Lumen.text, modifier = Modifier.size(22.dp))
        BrandMark(cell = 6.dp)
    }
}

@Composable
fun Chip(content: @Composable () -> Unit) {
    Row(
        Modifier.clip(CircleShape).background(Lumen.panelOpaque).border(1.dp, Lumen.line, CircleShape).padding(horizontal = 12.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) { content() }
}

@Composable
fun Label(text: String, size: TextUnit = 13.sp, color: Color = Lumen.text, weight: FontWeight = FontWeight.Normal, lines: Int = Int.MAX_VALUE, modifier: Modifier = Modifier) {
    Text(text, color = color, fontSize = size, fontWeight = weight, maxLines = lines, overflow = TextOverflow.Ellipsis, modifier = modifier)
}
