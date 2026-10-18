import LumenCore
import PhotosUI
import SwiftUI
import UIKit

/// The shell's half of the Crux bridge: forwards events to the core, shows its view, and
/// performs the operations it asks for. It decides nothing.
@MainActor
@Observable
final class AppModel {
    private(set) var view: ViewModel?
    private(set) var labels: [String: String] = [:]
    /// System bars hidden while a code fills the screen.
    private(set) var immersive = false
    var showFileImporter = false
    var showPhotoPicker = false
    var photoItem: PhotosPickerItem?

    @ObservationIgnored let core: LumenCore
    @ObservationIgnored let camera: CameraController
    @ObservationIgnored private let listener: Listener
    @ObservationIgnored private let device = DeviceControl()
    @ObservationIgnored private let decoder = JSONDecoder()
    @ObservationIgnored private var labelsLang = ""
    @ObservationIgnored private var pendingPick: UInt64 = 0
    @ObservationIgnored private var pendingImage: UInt64 = 0

    init() {
        let listener = Listener()
        let core = LumenCore(listener: listener)
        self.listener = listener
        self.core = core
        self.camera = CameraController(core: core)
        listener.model = self
        let locale = Locale.preferredLanguages.first ?? "en"
        dispatch(Ev.start(locale: locale))
        if let v = try? decoder.decode(ViewModel.self, from: Data(core.view().utf8)) {
            accept(v)
        }
        // heat: the core gives up the fast camera modes before iOS throttles the phone
        reportThermal()
        NotificationCenter.default.addObserver(forName: ProcessInfo.thermalStateDidChangeNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.reportThermal() }
        }
        #if DEBUG
        DebugScript.run(self)
        #endif
    }

    private func reportThermal() {
        dispatch(["type": "thermal", "level": ProcessInfo.processInfo.thermalState.rawValue])
    }

    func t(_ key: String) -> String { labels[key] ?? key }

    func dispatch(_ event: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: event), let json = String(data: data, encoding: .utf8) else { return }
        do {
            try core.dispatch(eventJson: json)
        } catch {
            print("[lumen] rejected event \(json): \(error)")
        }
    }

    fileprivate func apply(_ json: String) {
        let update: UpdateWire
        do {
            update = try decoder.decode(UpdateWire.self, from: Data(json.utf8))
        } catch {
            print("[lumen] undecodable update: \(error)")
            return
        }
        if update.view.rev >= (view?.rev ?? 0) {
            accept(update.view)
        }
        for r in update.requests {
            perform(r)
        }
    }

    private func accept(_ v: ViewModel) {
        if v.lang != labelsLang {
            labelsLang = v.lang
            if let dict = try? JSONSerialization.jsonObject(with: Data(lumenLabels(lang: v.lang).utf8)) as? [String: String] {
                labels = dict
            }
        }
        if v != view { view = v }
    }

    private func answer(_ id: UInt64, _ output: [String: Any]) {
        guard id != 0, let data = try? JSONSerialization.data(withJSONObject: output), let json = String(data: data, encoding: .utf8) else { return }
        try? core.resolve(id: id, outputJson: json)
    }

    private static func finished(_ ok: Bool, cancelled: Bool = false) -> [String: Any] {
        ["type": "finished", "ok": ok, "cancelled": cancelled]
    }

    // MARK: - operations

    private func perform(_ r: RequestWire) {
        let op = r.op
        switch op.type {
        case "load_prefs":
            answer(r.id, ["type": "prefs", "json": UserDefaults.standard.string(forKey: "lumen.prefs") ?? NSNull()])
        case "save_prefs":
            UserDefaults.standard.set(op.json, forKey: "lumen.prefs")
        case "keep_awake":
            device.keepAwake(op.on ?? false)
        case "brightness":
            device.boost(op.boost ?? false)
        case "immersive":
            immersive = op.on ?? false
        case "haptic":
            Haptics.play(op.kind ?? "light")
        case "camera_start":
            let id = r.id
            camera.start(device: Int(op.device ?? 0)) { [weak self] result in
                self?.answer(id, result)
            }
        case "camera_stop":
            camera.stop()
        case "camera_zoom":
            camera.setZoom(op.level ?? 1)
        case "camera_mode":
            camera.setMode(fps: Int(op.fps ?? 30), uhd: op.uhd ?? false)
        case "camera_exposure":
            break // iOS auto exposure keeps exposures short at 60 fps and above
        case "pick_file":
            pendingPick = r.id
            showFileImporter = true
        case "pick_image":
            pendingImage = r.id
            photoItem = nil
            showPhotoPicker = true
        case "save_file", "share_file", "open_file":
            let id = r.id
            guard let name = op.name, let bytes = core.receivedBytes() else {
                answer(id, Self.finished(false))
                return
            }
            let mime = op.mime ?? "application/octet-stream"
            let done: (Bool, Bool) -> Void = { [weak self] ok, cancelled in self?.answer(id, Self.finished(ok, cancelled: cancelled)) }
            switch op.type {
            case "save_file": Files.save(bytes, name: name, mime: mime, done: done)
            case "share_file": Files.share(bytes, name: name, done: done)
            default: Files.preview(bytes, name: name, done: done)
            }
        case "copy_text":
            if let s = core.copyText(pretty: op.pretty ?? true) {
                UIPasteboard.general.string = s
                answer(r.id, Self.finished(true))
            } else {
                answer(r.id, Self.finished(false))
            }
        case "open_url":
            if let s = op.url, let u = URL(string: s) { UIApplication.shared.open(u) }
        case "open_settings":
            if let u = URL(string: UIApplication.openSettingsURLString) { UIApplication.shared.open(u) }
        default:
            print("[lumen] unknown operation \(op.type)")
            answer(r.id, Self.finished(false))
        }
    }

    // MARK: - pickers

    func fileImported(_ result: Result<URL, Error>) {
        let id = pendingPick
        pendingPick = 0
        guard case .success(let url) = result else {
            answer(id, Self.finished(false))
            return
        }
        let core = self.core
        Task.detached(priority: .userInitiated) {
            let scoped = url.startAccessingSecurityScopedResource()
            defer { if scoped { url.stopAccessingSecurityScopedResource() } }
            let data = try? Data(contentsOf: url, options: .mappedIfSafe)
            if let data { core.loadFile(name: url.lastPathComponent, data: data) }
            await MainActor.run { [weak self] in self?.answer(id, Self.finished(data != nil)) }
        }
    }

    func fileImportCancelled() {
        answer(pendingPick, Self.finished(false, cancelled: true))
        pendingPick = 0
    }

    func photoPicked() {
        let id = pendingImage
        pendingImage = 0
        guard let item = photoItem else {
            answer(id, Self.finished(false, cancelled: true))
            return
        }
        photoItem = nil
        let core = self.core
        Task.detached(priority: .userInitiated) {
            let data = try? await item.loadTransferable(type: Data.self)
            let img = data.flatMap { Images.rgba(from: $0, maxSide: 2400) }
            if let img { core.scanImage(width: UInt32(img.width), height: UInt32(img.height), rgba: img.rgba) }
            let ok = img != nil
            await MainActor.run { [weak self] in self?.answer(id, Self.finished(ok)) }
        }
    }

    /// Files opened with Lumen from another app ("Open in…", share sheet).
    func open(url: URL) {
        let core = self.core
        Task.detached(priority: .userInitiated) {
            let scoped = url.startAccessingSecurityScopedResource()
            defer { if scoped { url.stopAccessingSecurityScopedResource() } }
            if let data = try? Data(contentsOf: url, options: .mappedIfSafe) {
                core.loadFile(name: url.lastPathComponent, data: data)
            }
        }
    }

    func scenePhase(_ active: Bool) {
        device.sceneActive(active)
        dispatch(Ev.appActive(active))
    }
}

/// Receives the core's updates on any thread and applies them on the main thread, in order.
private final class Listener: ShellListener, @unchecked Sendable {
    weak var model: AppModel?

    func onUpdate(updateJson: String) {
        DispatchQueue.main.async { [weak self] in
            MainActor.assumeIsolated {
                self?.model?.apply(updateJson)
            }
        }
    }
}
