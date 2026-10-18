import SwiftUI

/// The receiver (the web's recv.html): start screen, live camera with the dashboard, result.
struct ReceiveScreen: View {
    @Environment(AppModel.self) private var model
    let view: ViewModel

    var body: some View {
        let r = view.recv
        ZStack {
            Color.black.ignoresSafeArea()
            if r.cameraOn || r.cameraStarting {
                CameraPreview(model: model, overlay: r.overlay, zoom: r.zoom?.level)
                    .ignoresSafeArea()
            }
            if r.phase == "scanning" {
                VStack(spacing: 12) {
                    Spacer()
                    if let z = r.zoom {
                        Button { model.dispatch(Ev.resetZoom) } label: {
                            Text(z.label)
                                .font(Theme.mono(13, .semibold))
                                .foregroundStyle(Theme.text)
                                .padding(.horizontal, 12)
                                .frame(minHeight: 32)
                                .background(Capsule().fill(Color(red: 12 / 255, green: 16 / 255, blue: 23 / 255).opacity(0.86)))
                                .overlay(Capsule().stroke(z.active ? Theme.cyan : Theme.line2, lineWidth: 1))
                        }
                        .buttonStyle(.plain)
                        .accessibilityLabel(model.t("zoomHint"))
                    }
                    Hud(recv: r)
                }
                .padding(.horizontal, 10)
                .padding(.bottom, 6)
                .frame(maxWidth: 560)
                .frame(maxWidth: .infinity)
                if r.canSwitch {
                    HStack {
                        Spacer()
                        Button { model.dispatch(Ev.switchCamera) } label: { Image(systemName: "arrow.triangle.2.circlepath.camera") }
                            .buttonStyle(IconButtonStyle(background: Theme.panelOpaque))
                            .accessibilityLabel(model.t("switchCam"))
                    }
                    .padding(.trailing, 12)
                }
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
            if r.phase != "done" {
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
                    }
                    Spacer()
                }
                .padding(.horizontal, 14)
                .padding(.top, 8)
            }
        }
        .animation(.easeOut(duration: 0.2), value: r.phase)
    }
}

private struct Hud: View {
    @Environment(AppModel.self) private var model
    let recv: RecvVM

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .firstTextBaseline, spacing: 12) {
                Text(recv.name).font(.system(size: 15, weight: .semibold)).foregroundStyle(Theme.text).lineLimit(1).truncationMode(.middle)
                Spacer()
                Text(recv.pct).font(Theme.mono(22, .bold)).foregroundStyle(Theme.text).contentTransition(.numericText())
            }
            .padding(.bottom, 10)
            GeometryReader { g in
                ZStack(alignment: .leading) {
                    Capsule().fill(Color.white.opacity(0.08))
                    Capsule().fill(Theme.spectrum).frame(width: max(0, g.size.width * CGFloat(recv.progress)))
                        .animation(.easeOut(duration: 0.25), value: recv.progress)
                }
            }
            .frame(height: 8)
            HStack(alignment: .top, spacing: 8) {
                ForEach(recv.metrics, id: \.self) { m in
                    VStack(alignment: .leading, spacing: 1) {
                        Text(m.label).font(.system(size: 11.5)).foregroundStyle(Theme.faint).lineLimit(1)
                        Text(m.value).font(Theme.mono(14, .semibold)).foregroundStyle(Theme.text).lineLimit(1).minimumScaleFactor(0.8)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            .padding(.top, 12)
            Text(recv.tip)
                .font(.system(size: 13))
                .foregroundStyle(recv.tipOk ? Theme.muted : Theme.amber)
                .lineLimit(2)
                .padding(.top, 10)
                .frame(minHeight: 18, alignment: .leading)
            Text(recv.diag)
                .font(Theme.mono(11))
                .foregroundStyle(Theme.faint)
                .lineLimit(1)
                .truncationMode(.tail)
                .padding(.top, 6)
        }
        .padding(16)
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
                    FlowRow(spacing: 10) {
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
                        Button(model.t("fromImage")) { model.dispatch(Ev.pickImage) }.buttonStyle(LumenButtonStyle())
                    }
                    if let err = recv.error {
                        Text(err).font(.system(size: 13)).foregroundStyle(Theme.red).multilineTextAlignment(.center).padding(.top, 14)
                        if recv.canOpenSettings {
                            Button(model.t("openSettings")) { model.dispatch(Ev.openSettings) }
                                .buttonStyle(LumenButtonStyle(compact: true))
                                .padding(.top, 10)
                        }
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
