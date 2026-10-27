import LumenCore
import QuartzCore
import SwiftUI
import UIKit

/// The animated code. Per frame only a one-pixel-per-cell image changes (a few thousand
/// pixels); Core Animation scales it up with nearest-neighbour sampling and composites the
/// static mask (finders, markers, rounded cell gaps) on top. The CPU does no per-pixel work
/// at screen resolution, so a phone can play the code for a long time without heating up.
struct CodeView: UIViewRepresentable {
    let model: AppModel
    let geometry: GeometryVM
    let frameRev: UInt64
    let playing: Bool
    let fps: UInt32

    func makeUIView(context: Context) -> CodeUIView {
        let v = CodeUIView()
        v.core = model.core
        return v
    }

    func updateUIView(_ v: CodeUIView, context: Context) {
        v.update(geometry: geometry, frameRev: frameRev, playing: playing, fps: fps)
    }

    static func dismantleUIView(_ v: CodeUIView, coordinator: ()) {
        v.stop()
    }
}

final class CodeUIView: UIView {
    var core: LumenCore?
    private let cells = CALayer()
    private let maskLayer = CALayer()
    private var link: CADisplayLink?
    private var geometry: GeometryVM?
    private var frameRev: UInt64 = .max
    private var playing = false
    private var period: CFTimeInterval = 1.0 / 12
    private var nextFlip: CFTimeInterval = 0
    private var pending: CGImage?
    /// Frames are built off the main thread, one ahead; the display link only swaps images.
    private let builder = DispatchQueue(label: "lumen.code-frames", qos: .userInteractive)
    private var building = false
    private var fps: UInt32 = 12
    private static let srgb = CGColorSpace(name: CGColorSpace.sRGB)!

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = UIColor(red: 10 / 255, green: 13 / 255, blue: 20 / 255, alpha: 1)
        for l in [cells, maskLayer] {
            l.magnificationFilter = .nearest
            l.minificationFilter = .nearest
            l.actions = ["contents": NSNull(), "bounds": NSNull(), "position": NSNull()]
            layer.addSublayer(l)
        }
        cells.isOpaque = true
    }

    required init?(coder: NSCoder) { fatalError() }

    func update(geometry g: GeometryVM, frameRev rev: UInt64, playing: Bool, fps: UInt32) {
        period = 1.0 / Double(max(1, fps))
        if fps != self.fps {
            self.fps = fps
            link?.preferredFrameRateRange = Self.range(for: fps)
        }
        if rev != frameRev || g != geometry {
            frameRev = rev
            geometry = g
            reloadMask()
            setNeedsLayout()
            // show the first frame of the new stream at once
            prepare()
            flip()
            prepare()
        }
        if playing != self.playing {
            self.playing = playing
            playing ? start() : stopLink()
        }
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        guard let g = geometry else { return }
        let scale = window?.screen.scale ?? UIScreen.main.scale
        let size = CGSize(width: CGFloat(g.widthPx) / scale, height: CGFloat(g.heightPx) / scale)
        // whole device pixels, centred: every cell edge stays on a pixel edge
        let x = ((bounds.width - size.width) / 2 * scale).rounded() / scale
        let y = ((bounds.height - size.height) / 2 * scale).rounded() / scale
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        cells.frame = CGRect(origin: CGPoint(x: x, y: y), size: size)
        maskLayer.frame = cells.frame
        cells.contentsScale = scale
        maskLayer.contentsScale = scale
        CATransaction.commit()
        #if DEBUG
        DebugScript.frames["code"] = convert(cells.frame, to: nil)
        #endif
    }

    private func reloadMask() {
        guard let m = core?.sendMask() else { return }
        maskLayer.contents = Self.image(m.rgba, width: Int(m.width), height: Int(m.height), alpha: true)
    }

    /// Builds the next frame now (on a new stream, where it must be on screen at once).
    private func prepare() {
        guard let f = core?.sendFrame() else { return }
        pending = Self.image(f.rgba, width: Int(f.width), height: Int(f.height), alpha: false)
    }

    /// Builds the next frame in the background (while playing).
    private func prepareAhead() {
        guard !building, pending == nil, let core else { return }
        building = true
        let rev = frameRev
        builder.async { [weak self] in
            let f = core.sendFrame()
            let img = f.flatMap { Self.image($0.rgba, width: Int($0.width), height: Int($0.height), alpha: false) }
            DispatchQueue.main.async {
                guard let self else { return }
                self.building = false
                if self.frameRev == rev, self.pending == nil { self.pending = img }
            }
        }
    }

    private func flip() {
        guard let img = pending else { return }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        cells.contents = img
        CATransaction.commit()
        pending = nil
    }

    private func start() {
        guard link == nil else { return }
        let l = CADisplayLink(target: Proxy(self), selector: #selector(Proxy.tick(_:)))
        l.preferredFrameRateRange = Self.range(for: fps)
        l.add(to: .main, forMode: .common)
        link = l
        nextFlip = 0
    }

    /// 60 Hz is enough up to 30 fps; faster senders ask ProMotion screens for 120 Hz so every
    /// frame lasts the same number of refreshes.
    private static func range(for fps: UInt32) -> CAFrameRateRange {
        fps > 30 ? CAFrameRateRange(minimum: 60, maximum: 120, preferred: 120) : CAFrameRateRange(minimum: 30, maximum: 60, preferred: 60)
    }

    private func stopLink() {
        link?.invalidate()
        link = nil
    }

    func stop() {
        stopLink()
    }

    fileprivate func tick(_ l: CADisplayLink) {
        guard playing else { return }
        let now = l.targetTimestamp
        if nextFlip == 0 { nextFlip = now }
        // flip on the vsync closest to the schedule; keep the cadence, never bunch up
        if now + 0.002 < nextFlip {
            prepareAhead()
            return
        }
        nextFlip = max(nextFlip + period, now + period * 0.5)
        if pending == nil { prepare() } // the builder fell behind: build in place, never skip
        flip()
        prepareAhead()
    }

    private static func image(_ data: Data, width: Int, height: Int, alpha: Bool) -> CGImage? {
        guard let provider = CGDataProvider(data: data as CFData) else { return nil }
        let info: CGBitmapInfo = alpha
            ? CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue)
            : CGBitmapInfo(rawValue: CGImageAlphaInfo.noneSkipLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue)
        return CGImage(width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: width * 4, space: srgb,
                       bitmapInfo: info, provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)
    }

    /// Breaks the display link's strong reference to the view.
    private final class Proxy: NSObject {
        weak var view: CodeUIView?
        init(_ v: CodeUIView) { view = v }
        @objc func tick(_ l: CADisplayLink) {
            if let v = view { v.tick(l) } else { l.invalidate() }
        }
    }
}
