//! Two-language dictionary shared by every native shell. The keys and the wording are the
//! web demo's (`src/web/js/i18n.js`); the entries after the marked line exist only in the apps.
//!
//! The view model carries strings that change with state already translated; the static
//! labels go to a shell once per language through [`labels`].

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lang {
    Zh,
    #[default]
    En,
}

impl Lang {
    /// A BCP 47 tag or a stored value: Chinese for any `zh*`, English otherwise.
    pub fn from_tag(tag: &str) -> Lang {
        if tag.trim().to_ascii_lowercase().starts_with("zh") {
            Lang::Zh
        } else {
            Lang::En
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::Zh => "zh",
            Lang::En => "en",
        }
    }

    pub fn toggled(self) -> Lang {
        match self {
            Lang::Zh => Lang::En,
            Lang::En => Lang::Zh,
        }
    }
}

/// (key, Chinese, English)
const DICT: &[(&str, &str, &str)] = &[
    ("tagline", "用屏幕和相机传文件", "File transfer through screen and camera"),
    ("heroA", "把文件变成", "Turn files into"),
    ("heroB", "流动的光", "moving light"),
    ("sendTitle", "发送", "Send"),
    ("recvTitle", "接收", "Receive"),
    ("go", "开始 →", "Start →"),
    ("f1t", "自适应密度", "Adaptive density"),
    ("f1p", "根据屏幕物理尺寸选择格子大小，小屏手机也能被稳定识别。", "Cell size follows the physical screen size, so even small phone screens scan reliably."),
    ("f2t", "喷泉码", "Fountain coded"),
    ("f2p", "每帧都是新数据，丢帧、乱序都不影响，收够即可解出。", "Every frame carries new data. Dropped or out-of-order frames don’t matter."),
    ("f3t", "多色编码 + 纠错", "Colour + error correction"),
    ("f3p", "每格 2–3 bit，Reed–Solomon 纠错，逐包校验。", "2–3 bits per cell, Reed–Solomon, per-packet checksums."),
    ("home", "返回首页", "Back to home"),
    ("goSend", "要发送？去发送页", "Sending instead? Go to Send"),
    ("goRecv", "要接收？去接收页", "Receiving instead? Go to Receive"),
    ("zoomHint", "双指在画面上缩放；轻点恢复 1×", "Pinch the image to zoom; tap to reset to 1×"),
    ("dropTitle", "发送文件", "Send a file"),
    ("dropDesc", "拖入文件，或点击选择。", "Drop a file here, or click to choose."),
    ("choose", "选择文件", "Choose file"),
    ("sendText", "发送文本", "Send text"),
    ("dropHint", "调高屏幕亮度，接收端正对屏幕。", "Turn the brightness up; keep the receiver square to the screen."),
    ("flashWarn", "码每秒最多闪烁 60 次，对闪光敏感的人请勿注视。", "The code flashes up to 60 times a second. If flashing light affects you, don’t look at it."),
    ("textPh", "输入或粘贴文字", "Type or paste text"),
    ("density", "密度", "Density"),
    ("dAuto", "自动", "Auto"),
    ("dLow", "低", "Low"),
    ("dHigh", "高", "High"),
    ("dMax", "极高", "Max"),
    ("colors", "颜色", "Colours"),
    ("ecc", "纠错", "Error correction"),
    ("eLow", "低", "Low"),
    ("eMid", "中", "Mid"),
    ("eHigh", "高", "High"),
    ("speed", "帧率", "Frame rate"),
    ("perFrame", "每帧", "Per frame"),
    ("rate", "理论速率", "Raw rate"),
    ("loop", "一轮", "One pass"),
    ("grid", "网格", "Grid"),
    ("timeLeft", "剩余 {t}", "{t} left"),
    ("noData", "还没有收到数据", "Nothing received yet"),
    ("preparing", "正在准备…", "Preparing…"),
    ("paused", "已暂停", "Paused"),
    ("recvStartTitle", "扫描接收", "Scan to receive"),
    ("recvStartDesc", "相机对准发送端的码，保持稳定。", "Point the camera at the code and hold steady."),
    ("openCam", "打开相机", "Open camera"),
    ("fromImage", "从图片识别", "Decode an image"),
    ("camErr", "无法打开相机：", "Could not open camera: "),
    ("waiting", "等待识别…", "Looking for a code…"),
    ("receiving", "接收中", "Receiving"),
    ("speedM", "速度", "Speed"),
    ("etaM", "剩余", "ETA"),
    ("cellM", "格子", "Cell"),
    ("okM", "成功率", "Success"),
    ("tipSearch", "把彩色码完整放进画面", "Fit the whole code in view"),
    ("tipCloser", "再靠近一点，或在发送端降低密度", "Move closer, or lower the density on the sender"),
    ("tipSteady", "保持稳定，避免反光", "Hold steady and avoid glare"),
    ("tipColor", "颜色识别困难：试试发送端「颜色 4」或降低密度", "Colours are hard to read: try 4 colours or lower density on the sender"),
    ("tipGood", "识别良好", "Reading well"),
    ("tipLandscape", "码是横向的，试试横过手机", "The code is landscape — try rotating your phone"),
    ("save", "保存文件", "Save file"),
    ("open", "打开", "Open"),
    ("again", "再收一个", "Receive another"),
    ("checksum", "文件校验失败，已重新开始接收…", "File check failed, receiving again…"),
    ("recvTooLarge", "文件太大（{size}），本接收端最多接收 {max}", "File too large ({size}); this receiver accepts up to {max}"),
    ("hideDock", "隐藏控制栏（轻点画面可再次显示）", "Hide controls (tap the code to show them again)"),
    ("engine", "引擎", "Engine"),
    ("tabFile", "文件", "File"),
    ("tabText", "文本", "Text"),
    ("textKbd", "⌘/Ctrl + Enter 发送", "⌘/Ctrl + Enter to send"),
    ("chars", "字符", "chars"),
    ("textLabel", "文本", "Text"),
    ("textReceived", "收到文本", "Text received"),
    ("copy", "复制", "Copy"),
    ("copied", "已复制", "Copied"),
    ("openLink", "打开链接", "Open link"),
    ("saveAsFile", "保存为文件", "Save as file"),
    ("senderFps", "发送", "sender"),
    ("captured", "捕获", "captured"),
    ("dTime", "用时", "Time"),
    ("dSpeed", "平均速度", "Average speed"),
    ("dWire", "接收速率", "Receive rate"),
    ("dFrames", "新帧 / 识别次数", "New frames / scans"),
    ("dEngine", "引擎", "Engine"),
    ("dCode", "码", "Code"),
    ("dColours", "色", "colours"),
    ("truncatedHint", "仅显示前 {n} 个字符，复制或保存可获得完整内容", "Showing the first {n} characters; Copy or Save gets everything"),
    ("camFps", "相机", "cam"),
    ("scanFps", "识别", "scan"),
    ("frameFps", "新帧", "new frames"),
    ("tipFpsUp", "识别有余量：可把发送端帧率提高到约 {n}", "Spare capacity: you can raise the sender FPS to about {n}"),
    ("details", "参数", "Details"),
    ("vPretty", "格式化", "Formatted"),
    ("vRaw", "原文", "Raw"),
    ("wrap", "自动换行", "Wrap lines"),
    ("smaller", "缩小字号", "Smaller text"),
    ("larger", "放大字号", "Larger text"),
    ("lines", "行", "lines"),
    ("jsonReceived", "收到 JSON", "JSON received"),
    ("protocolLink", "协议规范", "Protocol specification"),
    ("guideLink", "开发者指南", "Developer guides"),
    // ---- apps only ----
    ("flashPlay", "知道了，开始播放", "I understand, play"),
    ("flashTitle", "光敏提示", "Photosensitivity"),
    ("heroP", "屏幕播放彩色码，相机拍下来还原文件。不需要网络。", "A screen plays the file as a colour code; a camera reads it back. No network needed."),
    ("sendDesc", "选文件或输入文字，屏幕开始播放。", "Pick a file or type text; the screen plays it."),
    ("recvDesc", "相机对准发送端的屏幕。", "Point the camera at the sender’s screen."),
    ("f4t", "一个 Rust 核心", "One Rust core"),
    ("f4p", "每个版本共用同一套 Rust 编解码，帧逐位一致；数据不离开设备。", "Every version runs the same Rust codec, with bit-identical frames; nothing leaves the device."),
    ("foot", "Lumen Code · 开放协议 · MIT 许可", "Lumen Code · open protocol · MIT licence"),
    ("buildLabel", "版本", "Version"),
    ("dropDescMobile", "选择要发送的文件。", "Choose a file to send."),
    ("dropHintMobile", "发送时屏幕保持常亮、最亮。", "The screen stays on at full brightness while sending."),
    ("paste", "粘贴", "Paste"),
    ("clear", "清空", "Clear"),
    ("share", "分享", "Share"),
    ("saved", "已保存", "Saved"),
    ("saveFailed", "保存失败", "Could not save"),
    ("camDenied", "没有相机权限。请在系统设置中允许 Lumen Code 使用相机。", "No camera permission. Allow Lumen Code to use the camera in Settings."),
    ("camNone", "没有找到可用的相机。", "No camera was found."),
    ("openSettings", "打开设置", "Open Settings"),
    ("switchCam", "切换相机", "Switch camera"),
    ("native", "Rust 原生", "Rust native"),
    ("imageNoCode", "这张图片里没有找到 Lumen 码。", "No Lumen code was found in this image."),
    ("readFailed", "无法读取这个文件。", "This file could not be read."),
    ("tooLarge", "文件太大（上限 {n}）。", "The file is too large (limit {n})."),
    ("settings", "设置", "Settings"),
    ("play", "播放", "Play"),
    ("pause", "暂停", "Pause"),
    ("close", "关闭", "Close"),
    ("fullscreen", "全屏", "Full screen"),
    ("language", "中 / EN", "中 / EN"),
    ("sendingNow", "正在发送", "Sending"),
    ("tipHot", "手机发热：相机已降到 60 帧，散热后再提高发送帧率", "The phone is warm: the camera is held at 60 fps; raise the sender FPS once it cools"),
    ("tipFpsDown", "这台相机每秒只拍 {cam} 帧：发送端调到约 {n} 帧反而更快", "This camera takes {cam} frames a second: about {n} fps on the sender is faster"),
    ("tipHotScan", "手机发热：已放慢识别，让它降温", "The phone is warm: scanning less often so it can cool down"),
    ("tipParallel", "让手机与屏幕平行：码的远端正在变小、变模糊", "Hold the phone parallel to the screen: the far side of the code is small and blurred"),
];

/// The translation of `key`, or the key itself when it is unknown.
pub fn t(lang: Lang, key: &str) -> &'static str {
    for (k, zh, en) in DICT {
        if *k == key {
            return match lang {
                Lang::Zh => zh,
                Lang::En => en,
            };
        }
    }
    // Unknown keys are a programming error (caught by the tests in app.rs).
    debug_assert!(false, "missing i18n key {key}");
    "?"
}

/// Every label in one language, for a shell to cache.
pub fn labels(lang: Lang) -> BTreeMap<&'static str, &'static str> {
    DICT.iter()
        .map(|(k, zh, en)| (*k, if lang == Lang::Zh { *zh } else { *en }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_complete() {
        let mut seen = std::collections::HashSet::new();
        for (k, zh, en) in DICT {
            assert!(seen.insert(*k), "duplicate key {k}");
            assert!(!zh.is_empty() && !en.is_empty(), "empty translation for {k}");
        }
        assert_eq!(t(Lang::Zh, "copy"), "复制");
        assert_eq!(Lang::from_tag("zh-Hans-CN"), Lang::Zh);
        assert_eq!(Lang::from_tag("en_US"), Lang::En);
    }
}
