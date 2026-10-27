package com.shelchin.lumen.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.shelchin.lumen.core.AppModel
import com.shelchin.lumen.core.Ev

@Composable
fun Root(model: AppModel) {
    MaterialTheme(colorScheme = darkColorScheme(background = Lumen.bg, surface = Lumen.bg, primary = Lumen.cyan)) {
        Box(Modifier.fillMaxSize().background(Lumen.bg)) {
            val v = model.view ?: return@Box
            Crossfade(targetState = v.screen, animationSpec = tween(180), label = "screen") { screen ->
                val cur = model.view ?: return@Crossfade
                when (screen) {
                    "send" -> SendScreen(model, cur)
                    "receive" -> ReceiveScreen(model, cur)
                    else -> HomeScreen(model)
                }
            }
            // back from a code (playing, paused or waiting on the warning) closes it and returns
            // to the pick screen (open settings close first); from anywhere else it goes home
            BackHandler(enabled = v.screen != "home") {
                val now = model.view
                if (now?.screen == "send" && now.send.phase != "pick" && now.send.settingsOpen && !now.send.flashAsk) model.dispatch(Ev.toggleSettings)
                else if (now?.screen == "send" && now.send.phase != "pick") model.dispatch(Ev.closeSend)
                else model.dispatch(Ev.navigate("home"))
            }
        }
    }
}

/** The landing screen: brand, hero, the two choices and the facts (the web's index.html). */
@Composable
fun HomeScreen(model: AppModel) {
    BoxWithConstraints(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
        val wide = maxWidth > 600.dp
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp)
                .widthIn(max = 980.dp),
        ) {
            Row(Modifier.fillMaxWidth().padding(top = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                BrandMark()
                Spacer(Modifier.padding(5.dp))
                Label("Lumen Code", size = 16.sp, weight = FontWeight.SemiBold)
                Spacer(Modifier.weight(1f))
                Box(
                    Modifier
                        .clip(RoundedCornerShape(9.dp))
                        .border(1.dp, Lumen.line, RoundedCornerShape(9.dp))
                        .clickable { model.dispatch(Ev.toggleLang) }
                        .padding(horizontal = 10.dp, vertical = 5.dp),
                ) { Label("中 / EN", size = 12.sp, color = Lumen.muted) }
            }
            Column(Modifier.padding(top = 48.dp, bottom = 30.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                Column {
                    Text(model.t("heroA"), color = Lumen.text, fontSize = 36.sp, fontWeight = FontWeight.Bold, lineHeight = 40.sp)
                    Text(model.t("heroB"), style = TextStyle(brush = Lumen.hero, fontSize = 36.sp, fontWeight = FontWeight.Bold, lineHeight = 40.sp))
                }
                Label(model.t("heroP"), size = 16.sp, color = Lumen.muted)
            }
            if (wide) {
                Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                    ChoiceCard(Icons.screen, Lumen.cyan, model.t("sendTitle"), model.t("sendDesc"), model.t("go"), Modifier.weight(1f)) { model.dispatch(Ev.navigate("send")) }
                    ChoiceCard(Icons.viewfinder, Lumen.pink, model.t("recvTitle"), model.t("recvDesc"), model.t("go"), Modifier.weight(1f)) { model.dispatch(Ev.navigate("receive")) }
                }
            } else {
                Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
                    ChoiceCard(Icons.screen, Lumen.cyan, model.t("sendTitle"), model.t("sendDesc"), model.t("go")) { model.dispatch(Ev.navigate("send")) }
                    ChoiceCard(Icons.viewfinder, Lumen.pink, model.t("recvTitle"), model.t("recvDesc"), model.t("go")) { model.dispatch(Ev.navigate("receive")) }
                }
            }
            Box(Modifier.padding(top = 36.dp).fillMaxWidth().height(1.dp).background(Lumen.line))
            val facts = listOf("f1t" to "f1p", "f2t" to "f2p", "f3t" to "f3p", "f4t" to "f4p")
            Column(Modifier.padding(top = 26.dp), verticalArrangement = Arrangement.spacedBy(18.dp)) {
                for (row in facts.chunked(2)) {
                    Row(horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                        for ((t, p) in row) {
                            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                                Label(model.t(t), size = 14.sp, weight = FontWeight.SemiBold)
                                Label(model.t(p), size = 13.sp, color = Lumen.muted)
                            }
                        }
                    }
                }
            }
            Label(model.t("foot"), size = 12.sp, color = Lumen.faint, modifier = Modifier.padding(top = 32.dp))
            // the version, for reports: app version and the commit of the sources
            Text(
                "${model.t("buildLabel")} ${com.shelchin.lumen.BuildConfig.VERSION_NAME} · ${model.view?.build ?: ""}",
                color = Lumen.faint,
                fontSize = 11.sp,
                fontFamily = androidx.compose.ui.text.font.FontFamily.Monospace,
                modifier = Modifier.padding(top = 6.dp, bottom = 16.dp),
            )
        }
    }
}

@Composable
private fun ChoiceCard(icon: ImageVector, tint: Color, title: String, desc: String, go: String, modifier: Modifier = Modifier, onClick: () -> Unit) {
    val src = remember { MutableInteractionSource() }
    val pressed by src.collectIsPressedAsState()
    val shape = RoundedCornerShape(20.dp)
    Column(
        modifier
            .fillMaxWidth()
            .scale(if (pressed) 0.98f else 1f)
            .clip(shape)
            .background(Brush.verticalGradient(listOf(Color.White.copy(alpha = 0.05f), Color.White.copy(alpha = 0.02f))))
            .border(1.dp, Lumen.line, shape)
            .clickable(interactionSource = src, indication = null, onClick = onClick)
            .padding(22.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        IconTile(icon, tint, size = 46.dp)
        Label(title, size = 20.sp, weight = FontWeight.SemiBold, modifier = Modifier.padding(top = 6.dp))
        Label(desc, size = 14.sp, color = Lumen.muted)
        Label(go, size = 14.sp, weight = FontWeight.SemiBold, modifier = Modifier.padding(top = 8.dp))
    }
}
