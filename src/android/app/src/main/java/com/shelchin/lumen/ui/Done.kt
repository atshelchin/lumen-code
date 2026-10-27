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
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.relocation.BringIntoViewRequester
import androidx.compose.foundation.relocation.bringIntoViewRequester
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.heightIn
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.isTraversalGroup
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.collapse
import androidx.compose.ui.semantics.expand
import androidx.compose.ui.semantics.traversalIndex
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.layout.FirstBaseline
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.shelchin.lumen.core.AppModel
import com.shelchin.lumen.core.DoneVM
import com.shelchin.lumen.core.Ev
import uniffi.lumen_ffi.JsonLine
import uniffi.lumen_ffi.JsonSpanKind
import uniffi.lumen_ffi.ReaderContent

/** The result: text and images fill the screen (reader); other files get a centred summary. */
@Composable
fun DoneScreen(model: AppModel, d: DoneVM) {
    if (d.reader) Reader(model, d) else Card(model, d)
}

/** The reader's small success mark, beside the name. */
@Composable
private fun Badge() {
    Box(Modifier.size(36.dp).clip(CircleShape).background(Lumen.green.copy(alpha = 0.14f)), contentAlignment = Alignment.Center) {
        Icon(Icons.check, null, tint = Lumen.green, modifier = Modifier.size(18.dp))
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

/**
 * Other files: one calm centred column under the top bar. A success mark, the name and one line
 * of numbers, then what to do with the file; the transfer's details fold away below.
 */
@Composable
private fun Card(model: AppModel, d: DoneVM) {
    Column(
        Modifier
            .fillMaxSize()
            .background(Color(0xD904060A))
            .windowInsetsPadding(WindowInsets.safeDrawing)
            .semantics { isTraversalGroup = true },
    ) {
        // home keeps the file: it is here again until "receive another".
        // TalkBack reads the result first, the way out after it.
        Box(Modifier.padding(horizontal = 10.dp, vertical = 8.dp).semantics { traversalIndex = 1f }) {
            HomeButton(model.t("home")) { model.dispatch(Ev.navigate("home")) }
        }
        BoxWithConstraints(Modifier.weight(1f).fillMaxWidth()) {
            val viewport = maxHeight
            // a phone on its side: one column would push the actions below the screen, so the
            // file stays on the left and what to do with it goes on the right
            val sideBySide = maxWidth > maxHeight && maxHeight < 480.dp
            if (sideBySide) {
                Row(
                    Modifier.fillMaxSize().padding(horizontal = 24.dp),
                    horizontalArrangement = Arrangement.spacedBy(32.dp, Alignment.CenterHorizontally),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f).widthIn(max = 420.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                        CardHeader(d, mark = 52.dp)
                    }
                    Column(
                        Modifier.weight(1f).widthIn(max = 420.dp).fillMaxHeight().verticalScroll(rememberScrollState()),
                        verticalArrangement = Arrangement.Center,
                    ) {
                        Spacer(Modifier.height(12.dp))
                        CardActions(model, d)
                        Spacer(Modifier.height(8.dp))
                        Details(model, d)
                        Spacer(Modifier.height(12.dp))
                    }
                }
            } else {
                Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
                    Column(
                        Modifier.fillMaxWidth().heightIn(min = viewport).padding(horizontal = 16.dp, vertical = 24.dp),
                        verticalArrangement = Arrangement.Center,
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        Column(Modifier.widthIn(max = 420.dp).fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
                            CardHeader(d, mark = 64.dp)
                            Spacer(Modifier.height(24.dp))
                            CardActions(model, d)
                            Spacer(Modifier.height(16.dp))
                            Details(model, d)
                        }
                    }
                }
            }
        }
    }
}

/** The check mark, the file name (two lines, shortened in the middle) and size · time · speed. */
@Composable
private fun CardHeader(d: DoneVM, mark: androidx.compose.ui.unit.Dp) {
    Box(Modifier.size(mark).clip(CircleShape).background(Lumen.green.copy(alpha = 0.14f)), contentAlignment = Alignment.Center) {
        Icon(Icons.check, null, tint = Lumen.green, modifier = Modifier.size(mark * 0.47f))
    }
    Spacer(Modifier.height(16.dp))
    MiddleEllipsis(
        d.title,
        TextStyle(color = Lumen.text, fontSize = 20.sp, fontWeight = FontWeight.SemiBold, textAlign = TextAlign.Center, lineHeight = 26.sp),
        maxLines = 2,
        modifier = Modifier.fillMaxWidth().semantics { heading() },
    )
    Spacer(Modifier.height(8.dp))
    Text(d.sub, color = Lumen.muted, fontSize = 14.sp, textAlign = TextAlign.Center, modifier = Modifier.fillMaxWidth())
}

@Composable
private fun CardActions(model: AppModel, d: DoneVM) {
    val tall = Modifier.heightIn(min = 48.dp)
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        LumenButton(d.saveLabel, primary = true, lines = 2, modifier = tall.fillMaxWidth()) { model.dispatch(Ev.save) }
        Row(Modifier.fillMaxWidth().height(IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            LumenButton(model.t("share"), lines = 2, modifier = tall.weight(1f).fillMaxHeight()) { model.dispatch(Ev.share) }
            if (d.canOpen) LumenButton(model.t("open"), lines = 2, modifier = tall.weight(1f).fillMaxHeight()) { model.dispatch(Ev.openFile) }
        }
        QuietButton(model.t("again"), Modifier.fillMaxWidth()) { model.dispatch(Ev.receiveAnother) }
    }
    d.note?.let {
        Text(
            it, color = if (d.noteOk) Lumen.green else Lumen.red, fontSize = 13.sp, textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth().padding(top = 8.dp).semantics { liveRegion = LiveRegionMode.Polite },
        )
    }
}

/** A text-style button: no fill, no border, for the action that leaves the result. */
@Composable
private fun QuietButton(label: String, modifier: Modifier = Modifier, onClick: () -> Unit) {
    val src = remember { MutableInteractionSource() }
    val pressed by src.collectIsPressedAsState()
    Box(
        modifier
            .clip(RoundedCornerShape(12.dp))
            .background(if (pressed) Color.White.copy(alpha = 0.06f) else Color.Transparent)
            .clickable(interactionSource = src, indication = null, role = Role.Button, onClick = onClick)
            .heightIn(min = 48.dp)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(label, color = if (pressed) Lumen.text else Lumen.muted, fontSize = 15.sp, fontWeight = FontWeight.Medium, textAlign = TextAlign.Center)
    }
}

/** "Details": folded by default; open, one full-width row per number and the code's settings under them. */
@Composable
private fun Details(model: AppModel, d: DoneVM) {
    Column(Modifier.fillMaxWidth()) {
        Row(
            Modifier
                .align(Alignment.CenterHorizontally)
                .clip(RoundedCornerShape(12.dp))
                .clickable(role = Role.Button, onClick = { model.dispatch(Ev.toggleStats) })
                .semantics(mergeDescendants = true) {
                    // TalkBack says "expanded" / "collapsed" from these
                    if (d.statsOpen) collapse { model.dispatch(Ev.toggleStats); true } else expand { model.dispatch(Ev.toggleStats); true }
                }
                .heightIn(min = 48.dp)
                .padding(horizontal = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Text(model.t("details"), color = Lumen.muted, fontSize = 14.sp, fontWeight = FontWeight.Medium)
            // grows with the label at large font sizes
            val icon = with(LocalDensity.current) { 18.sp.toDp() }
            Icon(Icons.chevronDown, null, tint = Lumen.muted, modifier = Modifier.size(icon).rotate(if (d.statsOpen) 180f else 0f))
        }
        // opened by a tap: scroll the rows into view (not when the result opens with them shown)
        val reveal = remember { BringIntoViewRequester() }
        var wasOpen by remember { mutableStateOf(d.statsOpen) }
        LaunchedEffect(d.statsOpen) {
            if (d.statsOpen && !wasOpen) reveal.bringIntoView()
            wasOpen = d.statsOpen
        }
        if (!d.statsOpen) return@Column
        Column(Modifier.fillMaxWidth().bringIntoViewRequester(reveal).padding(top = 8.dp)) {
            for ((i, m) in d.stats.withIndex()) {
                if (i > 0) Box(Modifier.fillMaxWidth().height(1.dp).background(Lumen.line))
                MetricRow(m.label, m.value)
            }
            d.codeLine?.let {
                Box(Modifier.fillMaxWidth().height(1.dp).background(Lumen.line))
                Text(it, color = Lumen.faint, fontSize = 12.sp, lineHeight = 17.sp, modifier = Modifier.fillMaxWidth().padding(top = 12.dp))
            }
        }
    }
}

/**
 * One number: label left, value flush right on the same line when both fit; otherwise the value
 * goes under the label (still flush right) and wraps. Nothing is cut off.
 */
@Composable
private fun MetricRow(label: String, value: String) {
    Layout(
        content = {
            Text(label, color = Lumen.muted, fontSize = 14.sp)
            Text(value, style = mono(14.sp, FontWeight.Medium), color = Lumen.text, textAlign = TextAlign.End)
        },
        modifier = Modifier.fillMaxWidth().semantics(mergeDescendants = true) {}.padding(vertical = 12.dp),
    ) { (l, v), c ->
        val w = c.maxWidth
        val gap = 16.dp.roundToPx()
        val loose = Constraints(maxWidth = w)
        val lp = l.measure(loose)
        val vp = v.measure(loose)
        if (lp.width + gap + vp.width <= w) {
            // same line, first baselines aligned
            val lb = lp[FirstBaseline]
            val vb = vp[FirstBaseline]
            val top = maxOf(lb, vb)
            val h = maxOf(top - lb + lp.height, top - vb + vp.height)
            layout(w, h) {
                lp.place(0, top - lb)
                vp.place(w - vp.width, top - vb)
            }
        } else {
            val between = 4.dp.roundToPx()
            layout(w, lp.height + between + vp.height) {
                lp.place(0, 0)
                vp.place(w - vp.width, lp.height + between)
            }
        }
    }
}

/**
 * Text that keeps its start and its end (a file name and its extension) and drops the middle
 * when it needs more than [maxLines] lines. TalkBack reads the whole text.
 */
@Composable
private fun MiddleEllipsis(text: String, style: TextStyle, maxLines: Int, modifier: Modifier = Modifier) {
    val measurer = rememberTextMeasurer()
    BoxWithConstraints(modifier) {
        val width = constraints.maxWidth
        val shown = remember(text, style, width, maxLines) {
            val c = Constraints(maxWidth = width)
            fun fits(s: String) = !measurer.measure(s, style, maxLines = maxLines, constraints = c).hasVisualOverflow
            if (fits(text)) text else {
                // the longest cut that fits: keep `n` characters, a little more of the start
                var lo = 0
                var hi = text.length
                while (lo < hi) {
                    val n = (lo + hi + 1) / 2
                    if (fits(cut(text, n))) lo = n else hi = n - 1
                }
                cut(text, lo)
            }
        }
        Text(shown, style = style, maxLines = maxLines, modifier = Modifier.fillMaxWidth().semantics { contentDescription = text })
    }
}

private fun cut(s: String, keep: Int): String {
    val head = (keep + 1) / 2
    return s.take(head).trimEnd() + "…" + s.takeLast(keep - head).trimStart()
}

@Composable
private fun Reader(model: AppModel, d: DoneVM) {
    Column(Modifier.fillMaxSize().background(Lumen.bg).windowInsetsPadding(WindowInsets.safeDrawing)) {
        Row(Modifier.padding(start = 6.dp, end = 16.dp, top = 4.dp, bottom = 10.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            // home keeps the file: it is here again until "receive another"
            HomeButton(model.t("home")) { model.dispatch(Ev.navigate("home")) }
            Badge()
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
