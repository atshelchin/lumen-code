package com.shelchin.lumen.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.unit.dp

/** The web demo's icons (inline SVG in the web pages), as vectors. */
object Icons {
    private fun icon(name: String, stroke: Float, vararg paths: String, filled: List<String> = emptyList()) =
        ImageVector.Builder(name, 24.dp, 24.dp, 24f, 24f).apply {
            for (p in paths) addPath(
                pathData = addPathNodes(p),
                stroke = SolidColor(Color.Black),
                strokeLineWidth = stroke,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            )
            for (p in filled) addPath(pathData = addPathNodes(p), fill = SolidColor(Color.Black))
        }.build()

    private fun circle(cx: Float, cy: Float, r: Float) = "M${cx - r},$cy a$r,$r 0 1,0 ${2 * r},0 a$r,$r 0 1,0 ${-2 * r},0"

    val chevronLeft = icon("back", 2.2f, "m14.5 6-6 6 6 6")
    val chevronDown = icon("down", 1.8f, "m6 9.5 6 6 6-6")
    val arrowRight = icon("arrow", 2f, "M5 12h14M13 6l6 6-6 6")
    val upload = icon("upload", 1.8f, "M12 15V4M7.5 8.5 12 4l4.5 4.5", "M4 14v3.5A2.5 2.5 0 0 0 6.5 20h11a2.5 2.5 0 0 0 2.5-2.5V14")
    val screen = icon(
        "screen", 1.8f,
        "M5.5 4h13A2.5 2.5 0 0 1 21 6.5v8a2.5 2.5 0 0 1-2.5 2.5h-13A2.5 2.5 0 0 1 3 14.5v-8A2.5 2.5 0 0 1 5.5 4z",
        "M8 21h8M12 17v4", "M9.5 10.5 12 8l2.5 2.5M12 8v6",
    )
    val viewfinder = icon(
        "scan", 1.8f,
        "M4 8V6a2 2 0 0 1 2-2h2M16 4h2a2 2 0 0 1 2 2v2M20 16v2a2 2 0 0 1-2 2h-2M8 20H6a2 2 0 0 1-2-2v-2",
        circle(12f, 12f, 3.2f),
    )
    val pause = icon("pause", 0f, filled = listOf("M7 5h2a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1z", "M15 5h2a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1h-2a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1z"))
    val play = icon("play", 0f, filled = listOf("M7 5.5v13a1 1 0 0 0 1.5.86l11-6.5a1 1 0 0 0 0-1.72l-11-6.5A1 1 0 0 0 7 5.5Z"))
    val sliders = icon("settings", 1.8f, "M4 7h10M18 7h2M4 17h4M12 17h8", circle(16f, 7f, 2f), circle(10f, 17f, 2f))
    val close = icon("close", 1.8f, "M6 6l12 12M18 6 6 18")
    val check = icon("check", 2.2f, "m5 12.5 4.5 4.5L19 7.5")
    val wrap = icon("wrap", 1.9f, "M4 6h16M4 12h13a3 3 0 0 1 0 6h-4", "m15 16-2 2 2 2M4 18h5")
    val switchCamera = icon(
        "switch", 1.8f,
        "M4 8.5A2.5 2.5 0 0 1 6.5 6H8l1.5-2h5L16 6h1.5A2.5 2.5 0 0 1 20 8.5v8a2.5 2.5 0 0 1-2.5 2.5h-11A2.5 2.5 0 0 1 4 16.5Z",
        "M9 12.5a3 3 0 0 1 5.2-2M15 12a3 3 0 0 1-5.2 2", "M14.5 8.8 14.2 10.6 16 10.4M9.5 15.2l.3-1.8-1.8.2",
    )
}
