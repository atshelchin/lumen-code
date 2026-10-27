import Foundation

// Swift mirrors of the core's view model (src/app/core/src/app/view.rs) and of the update
// envelope (src/app/core/src/runtime.rs). Field names are the core's camelCase JSON.

struct UpdateWire: Decodable {
    let view: ViewModel
    let requests: [RequestWire]
}

struct RequestWire: Decodable {
    /// 0 = a notification: perform, never answer.
    let id: UInt64
    let op: OpWire
}

/// Every shell operation, flattened: `type` says which one, the rest are its fields.
struct OpWire: Decodable {
    let type: String
    var json: String?
    var on: Bool?
    var boost: Bool?
    var kind: String?
    var device: UInt32?
    var level: Float?
    var fps: UInt32?
    var uhd: Bool?
    var name: String?
    var mime: String?
    var pretty: Bool?
    var url: String?
}

enum Screen: String, Decodable { case home, send, receive }

struct ViewModel: Decodable, Equatable {
    var rev: UInt64
    var lang: String
    var platform: String
    var screen: Screen
    var send: SendVM
    var recv: RecvVM
    var build: String
}

struct StatVM: Decodable, Equatable, Hashable {
    var label: String
    var value: String
    var wide: Bool
}

struct MetricVM: Decodable, Equatable, Hashable {
    var label: String
    var value: String
}

struct GeometryVM: Decodable, Equatable {
    var s: Int
    var x: Int
    var y: Int
    var widthPx: Int
    var heightPx: Int
}

struct SendVM: Decodable, Equatable {
    var phase: String // pick | preparing | ready
    var sourceTab: String // file | text
    var textMeta: String
    var canSendText: Bool
    var dropDesc: String
    var dropHint: String
    var flashWarn: String
    /// Ready but paused until the photosensitivity warning is acknowledged (`accept_flash`).
    var flashAsk: Bool
    /// The Text tab's text: the field shows this, so what is shown is what is sent.
    var text: String
    /// Bar: the file name, or just "Text" (never the text itself).
    var title: String
    /// Bar, under the title: size and time per pass ("78 KB · ≈ 3 s"), or "Paused".
    var subtitle: String
    /// Settings panel: raw rate, per frame, grid, bits per cell, engine time, in one line.
    var details: String
    var size: String
    var stats: [StatVM]
    var playing: Bool
    var fps: UInt32
    var settingsOpen: Bool
    var density: String
    var mode: UInt8
    var ecc: UInt8
    var geometry: GeometryVM?
    var frameRev: UInt64
    var error: String?
}

struct ZoomVM: Decodable, Equatable {
    var level: Float
    var min: Float
    var max: Float
    var label: String
    var active: Bool
}

struct OverlayVM: Decodable, Equatable {
    var points: [[Float]]
    var frameW: UInt32
    var frameH: UInt32
    var rotation: UInt32
    var tone: String // good | fair | poor
}

struct DoneVM: Decodable, Equatable {
    var kind: String? // message | json | text | image | file
    var reader: Bool
    var name: String
    var title: String
    var sub: String
    var stats: [MetricVM]
    var codeLine: String?
    var statsOpen: Bool
    var json: Bool
    var pretty: Bool
    var wrap: Bool
    var font: Int
    var copyLabel: String?
    var link: String?
    var saveLabel: String
    var savePrimary: Bool
    var canOpen: Bool
    var note: String?
    var noteOk: Bool
    var contentRev: UInt64
}

struct RecvVM: Decodable, Equatable {
    var phase: String // start | scanning | done
    var cameraOn: Bool
    var cameraStarting: Bool
    var error: String?
    var canOpenSettings: Bool
    var note: String?
    var live: Bool
    var status: String
    var name: String
    var pct: String
    var progress: Float
    /// Under the progress bar: "32 KB/s · 2s left" (empty until data arrives).
    var line: String
    /// Speed, time left, cell size, success: for the Details area.
    var metrics: [MetricVM]
    var tip: String
    var tipOk: Bool
    var diag: String
    var zoom: ZoomVM?
    var canSwitch: Bool
    var overlay: OverlayVM?
    var done: DoneVM?
}

/// Events, as the core's JSON vocabulary (src/app/core/src/app/mod.rs `Event`).
enum Ev {
    static func start(locale: String) -> [String: Any] { ["type": "start", "locale": locale, "platform": "ios"] }
    static func navigate(_ s: Screen) -> [String: Any] { ["type": "navigate", "screen": s.rawValue] }
    static let toggleLang: [String: Any] = ["type": "toggle_lang"]
    static func viewport(width: Double, height: Double, scale: Double, mmPerPx: Double) -> [String: Any] {
        ["type": "viewport", "widthPx": width, "heightPx": height, "scale": scale, "mmPerPx": mmPerPx]
    }
    static func appActive(_ on: Bool) -> [String: Any] { ["type": "app_active", "active": on] }
    static func sourceTab(_ t: String) -> [String: Any] { ["type": "source_tab", "tab": t] }
    static func textChanged(_ t: String) -> [String: Any] { ["type": "text_changed", "text": t] }
    static let pickFile: [String: Any] = ["type": "pick_file"]
    static let sendText: [String: Any] = ["type": "send_text"]
    static func density(_ v: String) -> [String: Any] { ["type": "set_density", "value": v] }
    static func mode(_ v: Int) -> [String: Any] { ["type": "set_mode", "value": v] }
    static func ecc(_ v: Int) -> [String: Any] { ["type": "set_ecc", "value": v] }
    static func fps(_ v: Int) -> [String: Any] { ["type": "set_fps", "value": v] }
    static let togglePlay: [String: Any] = ["type": "toggle_play"]
    static let acceptFlash: [String: Any] = ["type": "accept_flash"]
    static let toggleSettings: [String: Any] = ["type": "toggle_settings"]
    static let closeSend: [String: Any] = ["type": "close_send"]
    static let openCamera: [String: Any] = ["type": "open_camera"]
    static let switchCamera: [String: Any] = ["type": "switch_camera"]
    static func zoom(_ level: Float) -> [String: Any] { ["type": "set_zoom", "level": level] }
    static let resetZoom: [String: Any] = ["type": "reset_zoom"]
    static let pickImage: [String: Any] = ["type": "pick_image"]
    static let receiveAnother: [String: Any] = ["type": "receive_another"]
    static let copy: [String: Any] = ["type": "copy"]
    static let openLink: [String: Any] = ["type": "open_link"]
    static let save: [String: Any] = ["type": "save"]
    static let share: [String: Any] = ["type": "share"]
    static let openFile: [String: Any] = ["type": "open_file"]
    static let openSettings: [String: Any] = ["type": "open_settings"]
    static func jsonView(pretty: Bool) -> [String: Any] { ["type": "set_json_view", "pretty": pretty] }
    static let toggleWrap: [String: Any] = ["type": "toggle_wrap"]
    static func font(_ d: Int) -> [String: Any] { ["type": "set_font", "delta": d] }
    static let toggleStats: [String: Any] = ["type": "toggle_stats"]
}
