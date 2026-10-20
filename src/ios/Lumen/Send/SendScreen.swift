import SwiftUI

/// The sender (the web's send.html): pick a file or type text, then the code fills the screen
/// with a dock that hides itself while it plays.
struct SendScreen: View {
    @Environment(AppModel.self) private var model
    let view: ViewModel

    var body: some View {
        let s = view.send
        GeometryReader { proxy in
            ZStack {
                if s.phase == "pick" {
                    PickCard(send: s)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else {
                    Color(hex: 0x0a0d14).ignoresSafeArea()
                    if let g = s.geometry {
                        CodeView(model: model, geometry: g, frameRev: s.frameRev, playing: s.playing, fps: s.fps)
                    }
                }
            }
            .onAppear { report(proxy) }
            .onChange(of: proxy.size) { _, _ in report(proxy) }
        }
        .overlay(alignment: .topLeading) {
            if s.phase == "pick" {
                HomeButton { model.dispatch(Ev.navigate(.home)) }
                    .padding(.horizontal, 14)
                    .padding(.top, 8)
            }
        }
        .overlay(alignment: .bottom) {
            if s.phase != "pick" {
                Dock(send: s)
                    .padding(.horizontal, 8)
                    .padding(.bottom, 8)
                    .opacity(s.dockVisible ? 1 : 0)
                    .offset(y: s.dockVisible ? 0 : 16)
                    .allowsHitTesting(s.dockVisible)
                    .animation(.easeOut(duration: 0.25), value: s.dockVisible)
            }
        }
    }

    /// The safe area is the code's box; the core turns it into a grid.
    private func report(_ proxy: GeometryProxy) {
        let scale = UIScreen.main.scale
        let pad = UIDevice.current.userInterfaceIdiom == .pad
        // physical size of a point: ~0.16 mm on iPhones, ~0.19 mm on iPads
        let mmPerPt = pad ? 0.19 : 0.16
        model.dispatch(Ev.viewport(width: Double(proxy.size.width * scale), height: Double(proxy.size.height * scale),
                                   scale: Double(scale), mmPerPx: mmPerPt / Double(scale)))
    }
}

private struct PickCard: View {
    @Environment(AppModel.self) private var model
    let send: SendVM
    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        GeometryReader { outer in
            ScrollView {
                card.frame(maxWidth: .infinity, minHeight: outer.size.height)
            }
            .scrollDismissesKeyboard(.interactively)
            .scrollIndicators(.hidden)
        }
    }

    private var card: some View {
            VStack(spacing: 0) {
                Segmented(items: [("file", model.t("tabFile")), ("text", model.t("tabText"))], selected: send.sourceTab, minWidth: 84) {
                    model.dispatch(Ev.sourceTab($0))
                    Haptics.play("selection")
                }
                .padding(.bottom, 22)

                if send.sourceTab == "file" {
                    IconTile(symbol: "square.and.arrow.up", tint: Theme.cyan)
                    Text(model.t("dropTitle")).font(.system(size: 22, weight: .semibold)).foregroundStyle(Theme.text).padding(.top, 16)
                    Text(send.dropDesc).font(.system(size: 15)).foregroundStyle(Theme.muted).multilineTextAlignment(.center).padding(.top, 6).padding(.bottom, 22)
                    Button(model.t("choose")) { model.dispatch(Ev.pickFile) }.buttonStyle(LumenButtonStyle(primary: true))
                } else {
                    ZStack(alignment: .topLeading) {
                        TextEditor(text: $text)
                            .focused($focused)
                            .scrollContentBackground(.hidden)
                            .font(.system(size: 15))
                            .foregroundStyle(Theme.text)
                            .padding(10)
                            .frame(minHeight: 190, maxHeight: 320)
                            .onChange(of: text) { _, t in model.dispatch(Ev.textChanged(t)) }
                        if text.isEmpty {
                            Text(model.t("textPh")).font(.system(size: 15)).foregroundStyle(Theme.faint).padding(.horizontal, 15).padding(.vertical, 18).allowsHitTesting(false)
                        }
                    }
                    .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(Color.black.opacity(0.3)))
                    .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).stroke(focused ? Theme.cyan.opacity(0.55) : Theme.line2, lineWidth: 1))
                    HStack {
                        Text(send.textMeta).font(Theme.mono(12)).foregroundStyle(Theme.faint)
                        Spacer()
                        if text.isEmpty {
                            Button(model.t("paste")) {
                                if let s = UIPasteboard.general.string { text = s }
                            }
                            .font(.system(size: 12, weight: .semibold)).foregroundStyle(Theme.cyan)
                        } else {
                            Button(model.t("clear")) { text = "" }.font(.system(size: 12, weight: .semibold)).foregroundStyle(Theme.muted)
                        }
                    }
                    .padding(.horizontal, 2).padding(.top, 8).padding(.bottom, 16)
                    Button(model.t("sendText")) {
                        focused = false
                        model.dispatch(Ev.sendText)
                    }
                    .buttonStyle(LumenButtonStyle(primary: true))
                    .disabled(!send.canSendText)
                }

                if let err = send.error {
                    Text(err).font(.system(size: 13)).foregroundStyle(Theme.red).padding(.top, 14)
                }
                Text(send.dropHint).font(.system(size: 12.5)).foregroundStyle(Theme.faint).multilineTextAlignment(.center).padding(.top, 18)
                // photosensitivity warning, before any code plays
                HStack(alignment: .top, spacing: 8) {
                    Image(systemName: "exclamationmark.triangle").font(.system(size: 13, weight: .semibold)).foregroundStyle(Theme.amber)
                    Text(send.flashWarn).font(.system(size: 12.5)).foregroundStyle(Theme.warnText).multilineTextAlignment(.leading)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 10)
                .frame(maxWidth: 460, alignment: .leading)
                .background(RoundedRectangle(cornerRadius: 12).fill(Theme.amber.opacity(0.06)))
                .overlay(RoundedRectangle(cornerRadius: 12).stroke(Theme.amber.opacity(0.22), lineWidth: 1))
                .accessibilityElement(children: .combine)
                .padding(.top, 14)
                Button {
                    model.dispatch(Ev.navigate(.receive))
                } label: {
                    HStack(spacing: 6) {
                        Text(model.t("goRecv"))
                        Image(systemName: "arrow.right")
                    }
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(Theme.muted)
                }
                .buttonStyle(.plain)
                .padding(.top, 18)
            }
            .padding(.horizontal, 26)
            .padding(.top, 34)
            .padding(.bottom, 26)
            .frame(maxWidth: 520)
            .background(RoundedRectangle(cornerRadius: 24, style: .continuous).fill(Color.white.opacity(0.02)))
            .overlay(RoundedRectangle(cornerRadius: 24, style: .continuous).strokeBorder(Color.white.opacity(0.16), style: StrokeStyle(lineWidth: 1.5, dash: [6, 5])))
            .padding(.horizontal, 20)
            .padding(.top, 64)
            .padding(.bottom, 24)
    }
}

private struct Dock: View {
    @Environment(AppModel.self) private var model
    let send: SendVM

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 8) {
                Button { model.dispatch(Ev.togglePlay) } label: {
                    Image(systemName: send.playing ? "pause.fill" : "play.fill")
                }
                .buttonStyle(IconButtonStyle())
                .disabled(send.phase != "ready")

                HStack(spacing: 8) {
                    StatusDot(live: send.playing && send.phase == "ready", warn: !send.playing)
                    Text(send.title).font(.system(size: 14, weight: .semibold)).foregroundStyle(Theme.text).lineLimit(1).truncationMode(.middle)
                    Text(send.size).font(Theme.mono(12.5)).foregroundStyle(Theme.muted).lineLimit(1).fixedSize()
                }
                .frame(maxWidth: .infinity, alignment: .leading)

                Button { model.dispatch(Ev.toggleSettings) } label: { Image(systemName: "slider.horizontal.3") }
                    .buttonStyle(IconButtonStyle(active: send.settingsOpen))
                Button { model.dispatch(Ev.hideDock) } label: { Image(systemName: "chevron.down") }
                    .buttonStyle(IconButtonStyle())
                Button { model.dispatch(Ev.closeSend) } label: { Image(systemName: "xmark") }
                    .buttonStyle(IconButtonStyle())
            }
            ViewThatFits(in: .horizontal) {
                statsRow(send.stats)
                statsRow(send.stats.filter { !$0.wide })
            }
            .padding(.leading, 52)
            .padding(.top, 4)
            if send.settingsOpen {
                SettingsPanel(send: send)
                    .padding(.top, 10)
                    .overlay(alignment: .top) { Rectangle().fill(Theme.line).frame(height: 1) }
                    .padding(.top, 10)
            }
        }
        .padding(10)
        .frame(maxWidth: 720)
        .panel(opaque: true)
        .simultaneousGesture(TapGesture().onEnded { model.dispatch(Ev.dockActivity) })
    }

    private func statsRow(_ stats: [StatVM]) -> some View {
        HStack(spacing: 12) {
            ForEach(stats, id: \.self) { st in
                (Text(st.label + " ").foregroundStyle(Theme.muted) + Text(st.value).foregroundStyle(Theme.text).font(Theme.mono(12, .semibold)))
                    .font(.system(size: 12))
                    .lineLimit(1)
                    .fixedSize()
            }
        }
    }
}

private struct SettingsPanel: View {
    @Environment(AppModel.self) private var model
    let send: SendVM
    @State private var fps: Double = 12

    var body: some View {
        FlowRow(spacing: 14, alignCenter: false) {
            field("density") {
                Segmented(items: [("low", model.t("dLow")), ("auto", model.t("dAuto")), ("high", model.t("dHigh")), ("max", model.t("dMax"))], selected: send.density) {
                    model.dispatch(Ev.density($0))
                }
            }
            field("colors") {
                Segmented(items: [(1, "4"), (2, "8")], selected: Int(send.mode)) { model.dispatch(Ev.mode($0)) }
            }
            field("ecc") {
                Segmented(items: [(0, model.t("eLow")), (1, model.t("eMid")), (2, model.t("eHigh"))], selected: Int(send.ecc)) { model.dispatch(Ev.ecc($0)) }
            }
            field("speed") {
                HStack(spacing: 8) {
                    Slider(value: $fps, in: 4...60, step: 1) { editing in
                        if !editing { model.dispatch(Ev.dockActivity) }
                    }
                    .tint(Theme.cyan)
                    .frame(width: 130)
                    .onChange(of: fps) { _, v in model.dispatch(Ev.fps(Int(v))) }
                    Text("\(Int(fps))").font(Theme.mono(13, .bold)).foregroundStyle(Theme.text).frame(width: 24, alignment: .leading)
                }
            }
        }
        .onAppear { fps = Double(send.fps) }
    }

    private func field<C: View>(_ key: String, @ViewBuilder _ content: () -> C) -> some View {
        HStack(spacing: 8) {
            Text(model.t(key)).font(.system(size: 12)).foregroundStyle(Theme.faint)
            content()
        }
    }
}
