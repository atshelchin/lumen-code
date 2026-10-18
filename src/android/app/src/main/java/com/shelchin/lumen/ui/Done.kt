package com.shelchin.lumen.ui

import android.graphics.BitmapFactory
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredWidth
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.ParagraphStyle
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextIndent
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.shelchin.lumen.core.AppModel
import com.shelchin.lumen.core.DoneVM
import com.shelchin.lumen.core.Ev
import uniffi.lumen_ffi.JsonLine
import uniffi.lumen_ffi.JsonSpanKind
import uniffi.lumen_ffi.ReaderContent

/** The result: text and images fill the screen (reader); other files get a card. */
@Composable
fun DoneScreen(model: AppModel, d: DoneVM) {
    if (d.reader) Reader(model, d) else Card(model, d)
}

@Composable
private fun Badge(small: Boolean) {
    val s = if (small) 36.dp else 58.dp
    Box(Modifier.size(s).clip(CircleShape).background(Lumen.green.copy(alpha = 0.14f)), contentAlignment = Alignment.Center) {
        Icon(Icons.check, null, tint = Lumen.green, modifier = Modifier.size(if (small) 18.dp else 28.dp))
    }
}

@Composable
private fun Stats(d: DoneVM, modifier: Modifier = Modifier) {
    Column(
        modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(Color.White.copy(alpha = 0.03f))
            .border(1.dp, Lumen.line, RoundedCornerShape(12.dp))
            .padding(horizontal = 10.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        for (row in d.stats.chunked(3)) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                for (m in row) Column(Modifier.weight(1f)) {
                    Label(m.label, size = 11.sp, color = Lumen.faint, lines = 1)
                    Text(m.value, style = mono(13.5.sp, FontWeight.SemiBold), color = Lumen.text, maxLines = 1)
                }
                repeat(3 - row.size) { Spacer(Modifier.weight(1f)) }
            }
        }
        d.codeLine?.let { Label(it, size = 11.5.sp, color = Lumen.faint) }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Actions(model: AppModel, d: DoneVM) {
    val text = d.copyLabel != null
    FlowRow(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp, Alignment.CenterHorizontally), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        d.copyLabel?.let { LumenButton(it, primary = true) { model.dispatch(Ev.copy) } }
        if (d.link != null) LumenButton(model.t("openLink")) { model.dispatch(Ev.openLink) }
        LumenButton(d.saveLabel, primary = d.savePrimary && !text) { model.dispatch(Ev.save) }
        LumenButton(model.t("share")) { model.dispatch(Ev.share) }
        if (d.canOpen && !text && d.kind != "image") LumenButton(model.t("open")) { model.dispatch(Ev.openFile) }
        LumenButton(model.t("again")) { model.dispatch(Ev.receiveAnother) }
    }
    d.note?.let { Label(it, size = 12.5.sp, color = if (d.noteOk) Lumen.green else Lumen.red, modifier = Modifier.padding(top = 8.dp)) }
}

@Composable
private fun Card(model: AppModel, d: DoneVM) {
    Box(Modifier.fillMaxSize().background(Color(0xD904060A)).windowInsetsPadding(WindowInsets.safeDrawing), contentAlignment = Alignment.Center) {
        Column(
            Modifier.verticalScroll(rememberScrollState()).padding(14.dp).widthIn(max = 460.dp).panel().padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Badge(small = false)
            Text(d.title, color = Lumen.text, fontSize = 19.sp, fontWeight = FontWeight.SemiBold, textAlign = TextAlign.Center, modifier = Modifier.padding(top = 14.dp, bottom = 4.dp))
            Text(d.sub, color = Lumen.muted, fontSize = 13.5.sp, textAlign = TextAlign.Center)
            Stats(d, Modifier.padding(top = 16.dp))
            Box(Modifier.padding(top = 18.dp)) { Column(horizontalAlignment = Alignment.CenterHorizontally) { Actions(model, d) } }
        }
    }
}

@Composable
private fun Reader(model: AppModel, d: DoneVM) {
    Column(Modifier.fillMaxSize().background(Lumen.bg).windowInsetsPadding(WindowInsets.safeDrawing)) {
        Row(Modifier.padding(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 10.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Badge(small = true)
            Column(Modifier.weight(1f)) {
                Label(d.title, size = 16.sp, weight = FontWeight.SemiBold, lines = 1)
                Label(d.sub, size = 12.5.sp, color = Lumen.muted, lines = 1)
            }
            Row(
                Modifier
                    .clip(CircleShape)
                    .background(Color.White.copy(alpha = 0.04f))
                    .border(1.dp, Lumen.line2, CircleShape)
                    .clickable { model.dispatch(Ev.toggleStats) }
                    .height(34.dp)
                    .padding(start = 12.dp, end = 9.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Label(model.t("details"), size = 12.5.sp, color = Lumen.muted)
                Icon(Icons.chevronDown, null, tint = Lumen.muted, modifier = Modifier.size(16.dp).rotate(if (d.statsOpen) 180f else 0f))
            }
        }
        if (d.statsOpen) Stats(d, Modifier.padding(start = 16.dp, end = 16.dp, bottom = 10.dp))
        if (d.kind != "image") {
            Row(Modifier.padding(start = 16.dp, end = 16.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                if (d.json) Segmented(listOf(true to model.t("vPretty"), false to model.t("vRaw")), d.pretty) { model.dispatch(Ev.jsonView(it)) }
                Spacer(Modifier.weight(1f))
                ToolBtn(description = model.t("smaller"), onClick = { model.dispatch(Ev.font(-1)) }) { Label("A", size = 12.sp, color = Lumen.muted, weight = FontWeight.Bold) }
                ToolBtn(description = model.t("larger"), onClick = { model.dispatch(Ev.font(1)) }) { Label("A", size = 18.sp, color = Lumen.muted, weight = FontWeight.Bold) }
                if (d.kind != "message") ToolBtn(pressed = d.wrap, description = model.t("wrap"), onClick = { model.dispatch(Ev.toggleWrap) }) {
                    Icon(Icons.wrap, null, tint = if (d.wrap) Lumen.text else Lumen.muted, modifier = Modifier.size(18.dp))
                }
            }
        }
        Box(Modifier.weight(1f).fillMaxWidth().background(if (d.kind == "image") Color.Black else Color.Black.copy(alpha = 0.22f))) {
            Box(Modifier.fillMaxWidth().height(1.dp).background(Lumen.line))
            if (d.kind == "image") ZoomImage(model, d.contentRev) else ReaderText(model, d)
        }
        Column(Modifier.padding(start = 16.dp, end = 16.dp, top = 10.dp, bottom = 6.dp), horizontalAlignment = Alignment.CenterHorizontally) { Actions(model, d) }
    }
}

private val spanColors = mapOf(
    JsonSpanKind.KEY to Color(0xFF79D3FF),
    JsonSpanKind.STR to Color(0xFF9FE39A),
    JsonSpanKind.NUM to Color(0xFFFFCF5C),
    JsonSpanKind.BOOL to Color(0xFFFF7AB8),
    JsonSpanKind.NULL to Color(0xFFB9A4FF),
    JsonSpanKind.PUNCT to Color(0xFF6B7488),
)

@Composable
private fun ReaderText(model: AppModel, d: DoneVM) {
    val content: ReaderContent? = remember(d.contentRev, d.pretty) { model.core.reader(d.pretty) }
    content ?: return
    val plain = d.kind == "message"
    if (d.json && d.pretty && content.lines.isNotEmpty()) {
        JsonLines(content.lines, (13 + d.font).toFloat(), d.wrap)
        return
    }
    val text = content.text ?: return
    if (plain) {
        val size = (17 + d.font).toFloat()
        val annotated = remember(text, content.links) {
            buildAnnotatedString {
                append(text)
                for (l in content.links) {
                    val (a, b) = l.start.toInt() to l.end.toInt()
                    if (b <= text.length) addLink(
                        LinkAnnotation.Url(text.substring(a, b), TextLinkStyles(SpanStyle(color = Lumen.cyan, textDecoration = TextDecoration.Underline))),
                        a, b,
                    )
                }
            }
        }
        SelectionContainer {
            Text(
                annotated,
                color = Lumen.text,
                fontSize = size.sp,
                lineHeight = (size * 1.75f).sp,
                modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(start = 20.dp, end = 20.dp, top = 22.dp, bottom = 64.dp),
            )
        }
        return
    }
    // raw text: line by line, so megabytes scroll smoothly
    val lines = remember(text) { text.split('\n') }
    val size = (13 + d.font).toFloat()
    val hs = rememberScrollState()
    LazyColumn(
        Modifier.fillMaxSize().then(if (d.wrap) Modifier else Modifier.horizontalScroll(hs)),
        contentPadding = PaddingValues(start = 12.dp, end = 12.dp, top = 14.dp, bottom = 48.dp),
    ) {
        itemsIndexed(lines) { _, line ->
            Text(line, style = mono(size.sp), color = Lumen.text, softWrap = d.wrap, lineHeight = (size * 1.6f).sp)
        }
    }
}

/** Formatted JSON: line numbers in a gutter, indentation by depth, wrapped lines hanging two levels in. */
@Composable
private fun JsonLines(lines: List<JsonLine>, size: Float, wrap: Boolean) {
    val measurer = rememberTextMeasurer()
    val density = LocalDensity.current
    val ch = remember(size) { with(density) { measurer.measure("0", mono(size.sp)).size.width.toDp() } }
    val gut = ch * (lines.size.toString().length + 2)
    val hs = rememberScrollState()
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val widest = remember(lines) { lines.maxOf { l -> l.depth.toInt() * 2 + l.spans.sumOf { it.text.length } } }
        val contentWidth = if (wrap) maxWidth else maxOf(maxWidth, gut + ch * widest + 48.dp)
        LazyColumn(
            Modifier.fillMaxSize().then(if (wrap) Modifier else Modifier.horizontalScroll(hs)).then(if (wrap) Modifier else Modifier.requiredWidth(contentWidth)),
            contentPadding = PaddingValues(start = 4.dp, end = 12.dp, top = 14.dp, bottom = 48.dp),
        ) {
            itemsIndexed(lines) { i, line ->
                val d = line.depth.toInt()
                Row {
                    Text(
                        "${i + 1}", style = mono(size.sp).copy(lineHeight = (size * 1.6f).sp), color = Lumen.faint.copy(alpha = 0.6f), textAlign = TextAlign.End,
                        modifier = Modifier.width(gut - ch * 1.5f), maxLines = 1,
                    )
                    Spacer(Modifier.width(ch * 1.5f))
                    val text = remember(line) {
                        buildAnnotatedString {
                            withStyle(ParagraphStyle(lineHeight = (size * 1.6f).sp, textIndent = TextIndent(firstLine = (size * 0.6f * d * 2).sp, restLine = (size * 0.6f * (d + 2) * 2).sp))) {
                                for (sp in line.spans) withStyle(SpanStyle(color = spanColors[sp.kind] ?: Lumen.text)) { append(sp.text) }
                            }
                        }
                    }
                    Text(text, style = mono(size.sp).copy(lineHeight = (size * 1.6f).sp), softWrap = wrap)
                }
            }
        }
    }
}

@Composable
private fun ZoomImage(model: AppModel, rev: Long) {
    val bitmap = remember(rev) {
        model.core.receivedBytes()?.let { bytes ->
            val opts = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size, opts)
            var sample = 1
            while (maxOf(opts.outWidth, opts.outHeight) / sample > 4096) sample *= 2
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size, BitmapFactory.Options().apply { inSampleSize = sample })?.asImageBitmap()
        }
    } ?: return
    var scale by remember { mutableFloatStateOf(1f) }
    var offset by remember { mutableStateOf(Offset.Zero) }
    Image(
        bitmap, contentDescription = null, contentScale = ContentScale.Fit,
        modifier = Modifier
            .fillMaxSize()
            .pointerInput(Unit) {
                detectTransformGestures { _, pan, zoom, _ ->
                    scale = (scale * zoom).coerceIn(1f, 8f)
                    offset = if (scale == 1f) Offset.Zero else offset + pan
                }
            }
            .pointerInput(Unit) { detectTapGestures(onDoubleTap = { if (scale > 1f) { scale = 1f; offset = Offset.Zero } else scale = 2.5f }) }
            .graphicsLayer { scaleX = scale; scaleY = scale; translationX = offset.x; translationY = offset.y },
    )
}
