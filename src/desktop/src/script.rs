//! Drive the app from the LUMEN_SCRIPT environment variable, for tests without a mouse (as on
//! iOS and Android): nav:<screen>, lang, tab:<file|text>, text:<s>, sendtext, demo:<bytes>,
//! wait:<s>, tap, settings, play, camera, close, json, capture:<n>, feed, bench:<frames>.

use crate::app::Root;
use gpui::{AsyncWindowContext, WeakEntity};
use lumen_app::app::{Event, Screen, SourceTab};
use lumen_app::FrameRef;
use std::time::Duration;

pub async fn run(this: WeakEntity<Root>, cx: &mut AsyncWindowContext, script: String) {
    cx.background_executor().timer(Duration::from_millis(600)).await;
    let mut captured: Vec<(usize, usize, Vec<u8>)> = Vec::new();
    for raw in script.split(';') {
        let (cmd, arg) = raw.split_once(':').unwrap_or((raw, ""));
        eprintln!("[lumen-script] {raw}");
        let Ok(rt) = this.update(cx, |root, _| root.rt.clone()) else { return };
        match cmd {
            "nav" => rt.dispatch(Event::Navigate {
                screen: match arg {
                    "send" => Screen::Send,
                    "receive" => Screen::Receive,
                    _ => Screen::Home,
                },
            }),
            "lang" => rt.dispatch(Event::ToggleLang),
            "tab" => rt.dispatch(Event::SourceTab { tab: if arg == "text" { SourceTab::Text } else { SourceTab::File } }),
            "text" => rt.dispatch(Event::TextChanged { text: arg.into() }),
            "sendtext" => rt.dispatch(Event::SendText),
            "json" => {
                rt.dispatch(Event::TextChanged {
                    text: r#"{"name":"Lumen Code","version":1,"big":12345678901234567890,"list":[1,2,3,{"deep":true}],"url":"https://example.com/a?b=1","note":"把文件变成流动的光","nothing":null}"#.into(),
                });
                rt.dispatch(Event::SendText);
            }
            "demo" => {
                let n: usize = arg.parse().unwrap_or(20_000);
                let mut x: u32 = 0x9e3779b9;
                let bytes: Vec<u8> = (0..n)
                    .map(|_| {
                        x ^= x << 13;
                        x ^= x >> 17;
                        x ^= x << 5;
                        x as u8
                    })
                    .collect();
                rt.load_file(&format!("demo-{n}.bin"), bytes);
            }
            "wait" => cx.background_executor().timer(Duration::from_secs_f64(arg.parse().unwrap_or(1.0))).await,
            "tap" => rt.dispatch(Event::TapCode),
            "diag" => {
                let v = rt.view();
                let done = v.recv.done.as_ref().map_or(String::new(), |d| format!(" · done: {} {}", d.name, d.sub));
                eprintln!("[lumen-diag] {:?} {} {} · {} · {}{done}", v.recv.phase, v.recv.pct, v.recv.status, v.recv.diag, v.recv.tip);
            }
            "fps" => rt.dispatch(Event::SetFps { value: arg.parse().unwrap_or(30) }),
            "settings" => rt.dispatch(Event::ToggleSettings),
            "play" => rt.dispatch(Event::TogglePlay),
            "camera" => rt.dispatch(Event::OpenCamera),
            "close" => rt.dispatch(Event::CloseSend),
            "capture" => {
                captured.clear();
                for _ in 0..arg.parse().unwrap_or(4) {
                    let Some(f) = rt.send_frame() else { break };
                    let (w, h, k) = (f.width as usize, f.height as usize, 3);
                    let mut out = vec![0u8; w * k * h * k * 4];
                    for y in 0..h * k {
                        for x in 0..w * k {
                            let si = ((y / k) * w + x / k) * 4;
                            let di = (y * w * k + x) * 4;
                            out[di..di + 4].copy_from_slice(&f.rgba[si..si + 4]);
                        }
                    }
                    captured.push((w * k, h * k, out));
                }
            }
            "feed" => {
                for (w, h, rgba) in &captured {
                    rt.scan_image(&FrameRef::rgba(*w, *h, rgba));
                    cx.background_executor().timer(Duration::from_millis(80)).await;
                }
            }
            "bench" => {
                let n = arg.parse().unwrap_or(300);
                let summary = cx.background_executor().spawn(async move { lumen_app::engine::bench::run(n).summary() }).await;
                eprintln!("[lumen-bench] {summary}");
            }
            _ => eprintln!("[lumen-script] unknown command {raw}"),
        }
    }
}
