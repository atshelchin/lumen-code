import AVFoundation
import SwiftUI
import UIKit

/// The live camera picture (aspect fill), the detected code's outline, and pinch to zoom.
/// The picture is composited by the system straight from the camera; the app never draws it.
struct CameraPreview: UIViewRepresentable {
    let model: AppModel
    let overlay: OverlayVM?
    let zoom: Float?

    func makeUIView(context: Context) -> PreviewUIView {
        let v = PreviewUIView()
        v.previewLayer.session = model.camera.session
        v.previewLayer.videoGravity = .resizeAspectFill
        let camera = model.camera
        v.onRotation = { angle in camera.rotation = angle }
        camera.onDeviceChange = { [weak v] dev in v?.attach(device: dev) }
        if let dev = camera.device { v.attach(device: dev) }
        v.onZoom = { [weak model] z in model?.dispatch(Ev.zoom(z)) }
        return v
    }

    func updateUIView(_ v: PreviewUIView, context: Context) {
        v.zoomLevel = zoom
        v.show(overlay)
    }
}

final class PreviewUIView: UIView {
    override class var layerClass: AnyClass { AVCaptureVideoPreviewLayer.self }
    var previewLayer: AVCaptureVideoPreviewLayer { layer as! AVCaptureVideoPreviewLayer }
    var onRotation: ((UInt32) -> Void)?
    var onZoom: ((Float) -> Void)?
    var zoomLevel: Float?
    private let outline = CAShapeLayer()
    private var coordinator: AVCaptureDevice.RotationCoordinator?
    private var observation: NSKeyValueObservation?
    private var pinchStart: Float = 1
    private var last: OverlayVM?

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .black
        outline.fillColor = UIColor.clear.cgColor
        outline.lineWidth = 3
        outline.lineJoin = .round
        outline.shadowOpacity = 1
        outline.shadowRadius = 6
        outline.shadowOffset = .zero
        outline.actions = ["path": NSNull(), "strokeColor": NSNull(), "shadowColor": NSNull(), "opacity": NSNull()]
        layer.addSublayer(outline)
        addGestureRecognizer(UIPinchGestureRecognizer(target: self, action: #selector(pinched(_:))))
    }

    required init?(coder: NSCoder) { fatalError() }

    /// Keeps the picture upright as the phone turns (and tells the core how it is shown).
    func attach(device: AVCaptureDevice) {
        observation = nil
        let c = AVCaptureDevice.RotationCoordinator(device: device, previewLayer: previewLayer)
        coordinator = c
        apply(c.videoRotationAngleForHorizonLevelPreview)
        observation = c.observe(\.videoRotationAngleForHorizonLevelPreview, options: [.new]) { [weak self] c, _ in
            DispatchQueue.main.async { self?.apply(c.videoRotationAngleForHorizonLevelPreview) }
        }
    }

    private func apply(_ angle: CGFloat) {
        if let conn = previewLayer.connection, conn.isVideoRotationAngleSupported(angle) {
            conn.videoRotationAngle = angle
        }
        onRotation?(UInt32((Int(angle.rounded()) % 360 + 360) % 360))
        show(last)
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        outline.frame = bounds
        show(last)
    }

    func show(_ o: OverlayVM?) {
        last = o
        guard let o, o.points.count == 4, o.frameW > 0, o.frameH > 0 else {
            outline.path = nil
            return
        }
        let path = UIBezierPath()
        for (i, p) in o.points.enumerated() where p.count == 2 {
            // frame pixels -> device point of interest (0…1 in sensor orientation) -> layer
            let dp = CGPoint(x: CGFloat(p[0]) / CGFloat(o.frameW), y: CGFloat(p[1]) / CGFloat(o.frameH))
            let lp = previewLayer.layerPointConverted(fromCaptureDevicePoint: dp)
            if i == 0 { path.move(to: lp) } else { path.addLine(to: lp) }
        }
        path.close()
        let color: UIColor = switch o.tone {
        case "good": UIColor(red: 43 / 255, green: 220 / 255, blue: 116 / 255, alpha: 1)
        case "fair": UIColor(red: 1, green: 214 / 255, blue: 10 / 255, alpha: 1)
        default: UIColor(red: 1, green: 84 / 255, blue: 104 / 255, alpha: 1)
        }
        outline.path = path.cgPath
        outline.strokeColor = color.cgColor
        outline.shadowColor = color.cgColor
    }

    @objc private func pinched(_ g: UIPinchGestureRecognizer) {
        switch g.state {
        case .began: pinchStart = zoomLevel ?? 1
        case .changed: onZoom?(pinchStart * Float(g.scale))
        default: break
        }
    }
}
