import Photos
import QuickLook
import UIKit
import UniformTypeIdentifiers

// Platform services the core asks for by name: haptics, the screen, files.

@MainActor
enum Haptics {
    private static let light = UIImpactFeedbackGenerator(style: .light)
    private static let medium = UIImpactFeedbackGenerator(style: .medium)
    private static let selection = UISelectionFeedbackGenerator()
    private static let notification = UINotificationFeedbackGenerator()

    static func play(_ kind: String) {
        switch kind {
        case "light": light.impactOccurred(intensity: 0.8)
        case "medium": medium.impactOccurred()
        case "selection": selection.selectionChanged()
        case "success": notification.notificationOccurred(.success)
        case "warning": notification.notificationOccurred(.warning)
        case "error": notification.notificationOccurred(.error)
        default: light.impactOccurred()
        }
    }
}

/// Screen kept on and at full brightness while a code is shown; restored on the way out.
@MainActor
final class DeviceControl {
    private var awake = false
    private var boosted = false
    private var savedBrightness: CGFloat?

    private var screen: UIScreen? {
        UIApplication.shared.connectedScenes.compactMap { ($0 as? UIWindowScene)?.screen }.first
    }

    func keepAwake(_ on: Bool) {
        awake = on
        UIApplication.shared.isIdleTimerDisabled = on
    }

    func boost(_ on: Bool) {
        boosted = on
        apply()
    }

    func sceneActive(_ active: Bool) {
        if active {
            apply()
            UIApplication.shared.isIdleTimerDisabled = awake
        } else if let saved = savedBrightness {
            // never leave the system at full brightness behind the person's back
            screen?.brightness = saved
            savedBrightness = nil
        }
    }

    private func apply() {
        guard let screen else { return }
        if boosted {
            if savedBrightness == nil { savedBrightness = screen.brightness }
            screen.brightness = 1
        } else if let saved = savedBrightness {
            screen.brightness = saved
            savedBrightness = nil
        }
    }
}

enum Images {
    struct RGBA { let width: Int; let height: Int; let rgba: Data }

    /// Decodes and downsamples an image to tightly packed RGBA (EXIF orientation applied).
    static func rgba(from data: Data, maxSide: Int) -> RGBA? {
        guard let src = CGImageSourceCreateWithData(data as CFData, nil) else { return nil }
        let opts: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceThumbnailMaxPixelSize: maxSide,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
        ]
        guard let cg = CGImageSourceCreateThumbnailAtIndex(src, 0, opts as CFDictionary) else { return nil }
        let (w, h) = (cg.width, cg.height)
        var buf = Data(count: w * h * 4)
        let ok = buf.withUnsafeMutableBytes { raw -> Bool in
            guard let ctx = CGContext(data: raw.baseAddress, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                      space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                      bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue)
            else { return false }
            ctx.interpolationQuality = .high
            ctx.draw(cg, in: CGRect(x: 0, y: 0, width: w, height: h))
            return true
        }
        return ok ? RGBA(width: w, height: h, rgba: buf) : nil
    }
}

/// Save, share and preview the received file.
@MainActor
enum Files {
    private static var holder: NSObject?

    private static var top: UIViewController? {
        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first { $0.activationState == .foregroundActive }
            ?? UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        var vc = scene?.keyWindow?.rootViewController
        while let p = vc?.presentedViewController { vc = p }
        return vc
    }

    /// The bytes as a file with its own name in a fresh temporary folder.
    static func temp(_ bytes: Data, name: String) -> URL? {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("received", isDirectory: true)
        try? FileManager.default.removeItem(at: dir)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let safe = name.replacingOccurrences(of: "/", with: "_")
        let url = dir.appendingPathComponent(safe.isEmpty ? "file" : safe)
        do {
            try bytes.write(to: url, options: .atomic)
            return url
        } catch {
            return nil
        }
    }

    static func save(_ bytes: Data, name: String, mime: String, done: @escaping (Bool, Bool) -> Void) {
        if mime.hasPrefix("image/"), mime != "image/svg+xml", let image = UIImage(data: bytes) {
            PHPhotoLibrary.requestAuthorization(for: .addOnly) { status in
                guard status == .authorized || status == .limited else {
                    DispatchQueue.main.async { exportToFiles(bytes, name: name, done: done) }
                    return
                }
                PHPhotoLibrary.shared().performChanges({
                    PHAssetChangeRequest.creationRequestForAsset(from: image)
                }) { ok, _ in
                    DispatchQueue.main.async { done(ok, false) }
                }
            }
            return
        }
        exportToFiles(bytes, name: name, done: done)
    }

    private static func exportToFiles(_ bytes: Data, name: String, done: @escaping (Bool, Bool) -> Void) {
        guard let url = temp(bytes, name: name), let top else { return done(false, false) }
        let picker = UIDocumentPickerViewController(forExporting: [url], asCopy: true)
        let delegate = PickerDelegate(done: done)
        picker.delegate = delegate
        holder = delegate
        top.present(picker, animated: true)
    }

    static func share(_ bytes: Data, name: String, done: @escaping (Bool, Bool) -> Void) {
        guard let url = temp(bytes, name: name), let top else { return done(false, false) }
        let sheet = UIActivityViewController(activityItems: [url], applicationActivities: nil)
        sheet.completionWithItemsHandler = { _, completed, _, error in
            done(completed, !completed && error == nil)
        }
        if let pop = sheet.popoverPresentationController {
            pop.sourceView = top.view
            pop.sourceRect = CGRect(x: top.view.bounds.midX, y: top.view.bounds.maxY - 80, width: 1, height: 1)
        }
        top.present(sheet, animated: true)
    }

    static func preview(_ bytes: Data, name: String, done: @escaping (Bool, Bool) -> Void) {
        guard let url = temp(bytes, name: name), let top else { return done(false, false) }
        let ql = QLPreviewController()
        let source = PreviewSource(url: url)
        ql.dataSource = source
        holder = source
        top.present(ql, animated: true) { done(true, false) }
    }

    private final class PickerDelegate: NSObject, UIDocumentPickerDelegate {
        let done: (Bool, Bool) -> Void
        init(done: @escaping (Bool, Bool) -> Void) { self.done = done }
        func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) { done(true, false) }
        func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) { done(false, true) }
    }

    private final class PreviewSource: NSObject, QLPreviewControllerDataSource {
        let url: URL
        init(url: URL) { self.url = url }
        func numberOfPreviewItems(in controller: QLPreviewController) -> Int { 1 }
        func previewController(_ controller: QLPreviewController, previewItemAt index: Int) -> QLPreviewItem { url as NSURL }
    }
}
