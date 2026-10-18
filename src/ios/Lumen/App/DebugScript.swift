#if DEBUG
import AVFoundation
import LumenCore
import SwiftUI
import UIKit

/// Debug builds only: drive the app from a launch environment variable, for device tests
/// without touching the screen.
///
///     LUMEN_SCRIPT="nav:send;demo:40000;wait:2;snap:send;tap;wait:0.5;snap:dock"
///
/// Commands: nav:<home|send|receive>, lang, tab:<file|text>, text:<s>, sendtext,
/// demo:<bytes>, wait:<seconds>, bench:<camera frames>, snap:<name> (PNG in Documents), tap, settings, play,
/// camera, close, event:<json>, json (sample JSON text), image (a generated PNG),
/// capture:<n> (keep n sender frames, upscaled), feed (decode the kept frames as images).
@MainActor
enum DebugScript {
    private static var captured: [(Int, Int, Data)] = []

    static func run(_ model: AppModel) {
        guard let script = ProcessInfo.processInfo.environment["LUMEN_SCRIPT"], !script.isEmpty else { return }
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(600))
            for raw in script.split(separator: ";") {
                let cmd = raw.split(separator: ":", maxSplits: 1).map(String.init)
                let arg = cmd.count > 1 ? cmd[1] : ""
                print("[lumen-script] \(raw)")
                switch cmd[0] {
                case "nav": model.dispatch(Ev.navigate(Screen(rawValue: arg) ?? .home))
                case "lang": model.dispatch(Ev.toggleLang)
                case "tab": model.dispatch(Ev.sourceTab(arg))
                case "text": model.dispatch(Ev.textChanged(arg))
                case "sendtext": model.dispatch(Ev.sendText)
                case "demo":
                    let n = max(1, Int(arg) ?? 20000)
                    var arr = [UInt8](repeating: 0, count: n)
                    var x: UInt32 = 0x9e3779b9
                    for i in 0..<n {
                        x ^= x << 13; x ^= x >> 17; x ^= x << 5
                        arr[i] = UInt8(truncatingIfNeeded: x)
                    }
                    model.core.loadFile(name: "demo-\(n).bin", data: Data(arr))
                case "wait": try? await Task.sleep(for: .milliseconds(Int((Double(arg) ?? 1) * 1000)))
                case "snap": snap(arg.isEmpty ? "snap" : arg)
                case "tap": model.dispatch(Ev.tapCode)
                case "settings": model.dispatch(Ev.toggleSettings)
                case "play": model.dispatch(Ev.togglePlay)
                case "camera": model.dispatch(Ev.openCamera)
                case "close": model.dispatch(Ev.closeSend)
                case "json":
                    let obj: [String: Any] = ["name": "Lumen Code", "version": 1, "big": 12345678901234567890 as Double, "list": [1, 2, 3, ["deep": true]],
                                              "url": "https://example.com/a?b=1", "note": "把文件变成流动的光", "nothing": NSNull()]
                    let data = try! JSONSerialization.data(withJSONObject: obj, options: [.sortedKeys])
                    let text = String(data: data, encoding: .utf8)!
                    model.dispatch(Ev.textChanged(text))
                    model.dispatch(Ev.sendText)
                case "image":
                    let img = UIGraphicsImageRenderer(size: CGSize(width: 240, height: 160)).image { ctx in
                        UIColor(red: 0.04, green: 0.05, blue: 0.08, alpha: 1).setFill()
                        ctx.fill(CGRect(x: 0, y: 0, width: 240, height: 160))
                        let colors: [UIColor] = [.cyan, .systemPink, .yellow, .green]
                        for i in 0..<4 {
                            colors[i].setFill()
                            ctx.cgContext.fillEllipse(in: CGRect(x: 20 + i * 52, y: 50, width: 44, height: 44))
                        }
                    }
                    model.core.loadFile(name: "circles.png", data: img.pngData()!)
                case "capture":
                    captured = []
                    for _ in 0..<(Int(arg) ?? 4) {
                        guard let f = model.core.sendFrame() else { break }
                        captured.append(upscale(f.rgba, Int(f.width), Int(f.height), 3))
                    }
                    print("[lumen-script] captured \(captured.count) frames")
                case "feed":
                    for (w, h, rgba) in captured {
                        model.core.scanImage(width: UInt32(w), height: UInt32(h), rgba: rgba)
                        try? await Task.sleep(for: .milliseconds(80))
                    }
                case "formats":
                    // capture rates the back camera offers at 1080p and 4K (420 formats)
                    var lines: [String] = []
                    if let dev = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .back) {
                        for f in dev.formats {
                            let d = CMVideoFormatDescriptionGetDimensions(f.formatDescription)
                            let sub = CMFormatDescriptionGetMediaSubType(f.formatDescription)
                            guard d.width == 1920 || d.width == 3840 else { continue }
                            let fps = f.videoSupportedFrameRateRanges.map { Int($0.maxFrameRate) }.max() ?? 0
                            lines.append("\(d.width)x\(d.height) \(String(format: "%08x", sub)) max \(fps) fps binned=\(f.isVideoBinned)")
                        }
                    }
                    let dir = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
                    try? lines.joined(separator: "\n").write(to: dir.appendingPathComponent("formats.txt"), atomically: true, encoding: .utf8)
                case "dump":
                    let dir = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
                    try? model.core.view().write(to: dir.appendingPathComponent("view-\(arg.isEmpty ? "dump" : arg).json"), atomically: true, encoding: .utf8)
                case "grab":
                    model.camera.grab(Int(arg) ?? 6)
                case "bench":
                    let n = UInt32(arg) ?? 300
                    let thermal = ["nominal", "fair", "serious", "critical"][ProcessInfo.processInfo.thermalState.rawValue]
                    let result = "thermal \(thermal) · " + (await Task.detached { lumenSelftest(cameraFrames: n) }.value)
                    print("[lumen-bench] \(result)")
                    let dir = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
                    try? result.write(to: dir.appendingPathComponent("bench.txt"), atomically: true, encoding: .utf8)
                case "event":
                    if let o = try? JSONSerialization.jsonObject(with: Data(arg.utf8)) as? [String: Any] { model.dispatch(o) }
                default: print("[lumen-script] unknown command \(raw)")
                }
            }
        }
    }

    private static func upscale(_ src: Data, _ w: Int, _ h: Int, _ k: Int) -> (Int, Int, Data) {
        let s = [UInt8](src)
        var out = [UInt8](repeating: 0, count: w * k * h * k * 4)
        for y in 0..<(h * k) {
            for x in 0..<(w * k) {
                let si = ((y / k) * w + x / k) * 4
                let di = (y * w * k + x) * 4
                out[di] = s[si]; out[di + 1] = s[si + 1]; out[di + 2] = s[si + 2]; out[di + 3] = 255
            }
        }
        return (w * k, h * k, Data(out))
    }

    static func snap(_ name: String) {
        guard let window = UIApplication.shared.connectedScenes.compactMap({ ($0 as? UIWindowScene)?.keyWindow }).first else { return }
        let image = UIGraphicsImageRenderer(bounds: window.bounds).image { _ in
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        }
        let dir = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        try? image.pngData()?.write(to: dir.appendingPathComponent("snap-\(name).png"))
    }
}
#endif
