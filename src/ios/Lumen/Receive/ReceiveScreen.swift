import SwiftUI

/// The receiver (the web's recv page): start screen, live camera with a small panel, result.
/// While scanning, nothing covers the middle of the picture: the back button and status at
/// the top, the panel (and the camera tools just above it) at the bottom, and corner guides
/// between them until a code is found.
struct ReceiveScreen: View {
    @Environment(AppModel.self) private var model
    let view: ViewModel
    /// The bottom panel's height with the camera tools: the guides end above it.
    @State private var dockHeight: CGFloat = 180

    private static let dockBottom: CGFloat = 6

    var body: some View {
        let r = view.recv
        ZStack {
            Color.black.ignoresSafeArea()
            if r.cameraOn || r.cameraStarting {
                CameraPreview(model: model, overlay: r.overlay, zoom: r.zoom?.level)
                    .ignoresSafeArea()
            }
            if r.phase == "scanning" {
                // where to put the code; once one is found its outline takes over
                Guides()
                    .padding(.top, 62)
                    .padding(.bottom, dockHeight + Self.dockBottom + 18)
                    .opacity(r.live ? 0 : 1)
                    .animation(.easeOut(duration: 0.3), value: r.live)
                    .debugFrame("guides")
                VStack(spacing: 0) {
                    Spacer(minLength: 0)
                    VStack(spacing: 10) {
                        if r.zoom != nil || r.canSwitch {
                            CamTools(recv: r)
                        }
                        Hud(recv: r)
                            .debugFrame("panel")
                    }
                    .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { dockHeight = $0 }
                }
                .padding(.horizontal, 10)
                .padding(.bottom, Self.dockBottom)
                .frame(maxWidth: 560)
                .frame(maxWidth: .infinity)
            }
            if r.phase == "start" {
                StartPane(recv: r)
            }
            if r.phase == "done", let d = r.done {
                DoneScreen(done: d)
                    .transition(.opacity.combined(with: .scale(scale: 0.98)))
            }
        }
        .overlay(alignment: .top) {
            // the done screen keeps the way out (the core keeps the result until "receive
            // another"); the full-screen reader carries the button in its own header
            if r.phase != "done" || r.done?.reader == false {
                HStack(spacing: 10) {
                    HomeButton { model.dispatch(Ev.navigate(.home)) }
                    if r.phase == "scanning" {
                        HStack(spacing: 8) {
                            StatusDot(live: r.live)
                            Text(r.status).font(.system(size: 13, weight: .semibold)).foregroundStyle(Theme.text).lineLimit(1)
                        }
                        .padding(.horizontal, 12)
                        .padding(.vertical, 6)
                        .background(Capsule().fill(Theme.panelOpaque))
                        .overlay(Capsule().stroke(Theme.line, lineWidth: 1))
                        .accessibilityElement(children: .combine)
                    }
                    Spacer()
                }
                .padding(.horizontal, 14)
                .padding(.top, 8)
                .debugFrame("topbar")
            }
        }
        .animation(.easeOut(duration: 0.2), value: r.phase)
    }
}

/// Four corner brackets: the frame to put the sender's code in.
private struct Guides: View {
    var body: some View {
        GeometryReader { g in
            let inset = max(22, g.size.width * 0.05)
            if g.size.height > 70 && g.size.width > 2 * inset + 70 {
                GuideCorners()
                    .stroke(Color.white.opacity(0.7), style: StrokeStyle(lineWidth: 3, lineCap: .butt))
                    .shadow(color: .black.opacity(0.6), radius: 3)
                    .padding(.horizontal, inset)
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

private struct GuideCorners: Shape {
    var arm: CGFloat = 30
    var radius: CGFloat = 10

    func path(in rect: CGRect) -> Path {
        let r = rect.insetBy(dx: 1.5, dy: 1.5)
        var p = Path()
        // each corner: along one edge into the corner, round it, out along the other
        let corners: [(CGPoint, CGPoint, CGPoint)] = [
            (CGPoint(x: r.minX, y: r.minY + arm), CGPoint(x: r.minX, y: r.minY), CGPoint(x: r.minX + arm, y: r.minY)),
            (CGPoint(x: r.maxX - arm, y: r.minY), CGPoint(x: r.maxX, y: r.minY), CGPoint(x: r.maxX, y: r.minY + arm)),
            (CGPoint(x: r.maxX, y: r.maxY - arm), CGPoint(x: r.maxX, y: r.maxY), CGPoint(x: r.maxX - arm, y: r.maxY)),
            (CGPoint(x: r.minX + arm, y: r.maxY), CGPoint(x: r.minX, y: r.maxY), CGPoint(x: r.minX, y: r.maxY - arm)),
        ]
        for (a, corner, b) in corners {
            p.move(to: a)
            p.addArc(tangent1End: corner, tangent2End: b, radius: radius)
            p.addLine(to: b)
        }
        return p
    }
}

/// Zoom and camera switch, at the edge of the picture just above the panel.
private struct CamTools: View {
    @Environment(AppModel.self) private var model
    let recv: RecvVM

    var body: some View {
        HStack(spacing: 8) {
            Spacer()
            if let z = recv.zoom {
                Button { model.dispatch(Ev.resetZoom) } label: {
                    Text(z.label)
                        .font(Theme.mono(13, .semibold))
                        .foregroundStyle(Theme.text)
                        .padding(.horizontal, 14)
                        .frame(minWidth: 56, minHeight: 44)
                        .background(Capsule().fill(Theme.panelOpaque))
                        .overlay(Capsule().stroke(z.active ? Theme.cyan : Theme.line2, lineWidth: 1))
                }
                .buttonStyle(.plain)
                .accessibilityLabel(model.t("zoomHint"))
                .accessibilityValue(z.label)
            }
            if recv.canSwitch {
                Button { model.dispatch(Ev.switchCamera) } label: { Image(systemName: "arrow.triangle.2.circlepath.camera") }
                    .buttonStyle(IconButtonStyle(background: Theme.panelOpaque))
                    .accessibilityLabel(model.t("switchCam"))
            }
        }
    }
}

/// Name and percentage, progress, speed and time left, one tip; the rest under Details.
private struct Hud: View {
    @Environment(AppModel.self) private var model
    let recv: RecvVM
    /// Folded at first; the choice is remembered.
    @AppStorage("lumen.recv.details") private var details = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .firstTextBaseline, spacing: 12) {
                Text(recv.name).font(.system(size: 15, weight: .semibold)).foregroundStyle(Theme.text).lineLimit(1).truncationMode(.middle)
                Spacer()
                Text(recv.pct).font(Theme.mono(22, .bold)).foregroundStyle(Theme.text).contentTransition(.numericText())
            }
            .padding(.bottom, 10)
            .accessibilityElement(children: .combine)
            GeometryReader { g in
                ZStack(alignment: .leading) {
                    Capsule().fill(Color.white.opacity(0.08))
                    Capsule().fill(Theme.spectrum).frame(width: max(0, g.size.width * CGFloat(recv.progress)))
                        .animation(.easeOut(duration: 0.25), value: recv.progress)
                }
            }
            .frame(height: 8)
            .accessibilityHidden(true)
            // keeps its height while the line is empty (the Details button holds it)
            HStack(spacing: 10) {
                Text(recv.line)
                    .font(.system(size: 13.5))
                    .monospacedDigit()
                    .foregroundStyle(Theme.text)
                    .lineLimit(1)
                Spacer(minLength: 0)
                Button {
                    withAnimation(.easeOut(duration: 0.2)) { details.toggle() }
                } label: {
                    HStack(spacing: 4) {
                        Text(model.t("details"))
                        Image(systemName: "chevron.down")
                            .font(.system(size: 11, weight: .semibold))
                            .rotationEffect(.degrees(details ? 180 : 0))
                    }
                    .font(.system(size: 12.5))
                    .foregroundStyle(Theme.muted)
                    .padding(.leading, 12)
                    .padding(.trailing, 10)
                    .frame(minHeight: 34)
                    .background(Capsule().fill(Color.white.opacity(0.04)))
                    .overlay(Capsule().stroke(Theme.line2, lineWidth: 1))
                    .padding(.vertical, 5)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(model.t("details"))
                .accessibilityAddTraits(details ? .isSelected : [])
            }
            .padding(.top, 4)
            Text(recv.tip)
                .font(.system(size: 13))
                .foregroundStyle(recv.tipOk ? Theme.muted : Theme.amber)
                .lineLimit(1)
                .frame(minHeight: 18, alignment: .leading)
            if details {
                VStack(alignment: .leading, spacing: 8) {
                    FlowRow(spacing: 18, alignCenter: false) {
                        ForEach(recv.metrics, id: \.self) { m in
                            (Text(m.label + " ").foregroundStyle(Theme.muted) + Text(m.value).fontWeight(.semibold).monospacedDigit().foregroundStyle(Theme.text))
                                .font(.system(size: 12.5))
                                .lineLimit(1)
                        }
                    }
                    Text(recv.diag)
                        .font(Theme.mono(11))
                        .foregroundStyle(Theme.faint)
                        .lineLimit(2)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.top, 10)
                .overlay(alignment: .top) { Rectangle().fill(Theme.line).frame(height: 1) }
                .padding(.top, 10)
                .transition(.opacity)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 14)
        .panel(opaque: true)
    }
}

private struct StartPane: View {
    @Environment(AppModel.self) private var model
    let recv: RecvVM

    var body: some View {
        ZStack {
            Theme.bg.ignoresSafeArea()
            RadialGradient(colors: [Theme.pink.opacity(0.08), .clear], center: UnitPoint(x: 0.5, y: 0.3), startRadius: 0, endRadius: 420)
                .ignoresSafeArea()
            ScrollView {
                VStack(spacing: 0) {
                    IconTile(symbol: "viewfinder", tint: Theme.pink)
                    Text(model.t("recvStartTitle")).font(.system(size: 24, weight: .bold)).foregroundStyle(Theme.text).padding(.top, 18).padding(.bottom, 8)
                    Text(model.t("recvStartDesc")).font(.system(size: 15)).foregroundStyle(Theme.muted).multilineTextAlignment(.center).padding(.bottom, 22)
                    if let err = recv.error {
                        Text(err).font(.system(size: 13)).foregroundStyle(Theme.red).multilineTextAlignment(.center).padding(.bottom, 16)
                    }
                    FlowRow(spacing: 10) {
                        if recv.canOpenSettings {
                            // camera access denied: asking again does nothing, Settings is the way
                            Button(model.t("openSettings")) { model.dispatch(Ev.openSettings) }
                                .buttonStyle(LumenButtonStyle(primary: true))
                        } else {
                            Button {
                                model.dispatch(Ev.openCamera)
                            } label: {
                                HStack(spacing: 8) {
                                    if recv.cameraStarting { ProgressView().tint(Theme.ink).scaleEffect(0.8) }
                                    Text(model.t("openCam"))
                                }
                            }
                            .buttonStyle(LumenButtonStyle(primary: true))
                            .disabled(recv.cameraStarting)
                        }
                        Button(model.t("fromImage")) { model.dispatch(Ev.pickImage) }.buttonStyle(LumenButtonStyle())
                    }
                    if let note = recv.note {
                        Text(note).font(.system(size: 13)).foregroundStyle(Theme.amber).multilineTextAlignment(.center).padding(.top, 14)
                    }
                    Button {
                        model.dispatch(Ev.navigate(.send))
                    } label: {
                        HStack(spacing: 6) {
                            Text(model.t("goSend"))
                            Image(systemName: "arrow.right")
                        }
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(Theme.muted)
                    }
                    .buttonStyle(.plain)
                    .padding(.top, 26)
                }
                .frame(maxWidth: 420)
                .padding(24)
                .frame(maxWidth: .infinity)
                .padding(.top, 90)
            }
            .scrollIndicators(.hidden)
        }
    }
}
