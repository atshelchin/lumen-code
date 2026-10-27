import SwiftUI
import UIKit
import UniformTypeIdentifiers

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
                            .accessibilityHidden(s.flashAsk)
                    }
                    if s.flashAsk {
                        FlashAsk(warning: s.flashWarn)
                            .transition(.opacity)
                    }
                }
            }
            .animation(.easeOut(duration: 0.2), value: s.flashAsk)
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
            // hidden while the warning waits: it must not cover the card (Play there shows it)
            if s.phase != "pick" && !s.flashAsk {
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
    @FocusState private var focused: Bool

    /// The field edits its own copy and reports each change to the core (`send.text`), which
    /// arrives back a moment later. While the field is being edited its copy wins, so a late
    /// echo never undoes a keystroke or breaks an input method's composition; otherwise it
    /// adopts the core's text (e.g. after a text send is closed).
    @State private var text = ""
    private func setText(_ t: String) {
        text = t
        model.dispatch(Ev.textChanged(t))
    }

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
                            .onChange(of: text) { _, t in if t != send.text { model.dispatch(Ev.textChanged(t)) } }
                            .onChange(of: send.text) { _, t in if !focused && t != text { text = t } }
                            .onAppear { text = send.text }
                            .scrollContentBackground(.hidden)
                            .font(.system(size: 15))
                            .foregroundStyle(Theme.text)
                            .padding(10)
                            .frame(minHeight: 190, maxHeight: 320)
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
                            // the system paste control: reading the pasteboard on its tap needs no prompt
                            PasteControl { setText($0) }
                            .foregroundStyle(Theme.ink)
                        } else {
                            Button { setText("") } label: {
                                Text(model.t("clear"))
                                    .font(.system(size: 12, weight: .semibold)).foregroundStyle(Theme.muted)
                                    .frame(minWidth: 44, minHeight: 44)
                                    .contentShape(Rectangle())
                            }
                            .buttonStyle(.plain)
                            .padding(.vertical, -14) // a 44 pt target without a taller row
                        }
                    }
                    .frame(minHeight: 16)
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

/// Over the paused first code: the photosensitivity warning, which must be acknowledged
/// before anything flashes (the core keeps the answer).
private struct FlashAsk: View {
    @Environment(AppModel.self) private var model
    let warning: String
    @AccessibilityFocusState private var warningFocused: Bool

    var body: some View {
        ZStack {
            // dims the paused code and keeps its taps (which toggle the dock) away
            Color.black.opacity(0.55).ignoresSafeArea().contentShape(Rectangle()).onTapGesture {}
            GeometryReader { outer in
                ScrollView {
                    card
                        .frame(maxWidth: .infinity, minHeight: outer.size.height)
                }
                .scrollBounceBehavior(.basedOnSize)
                .scrollIndicators(.hidden)
            }
        }
        .onAppear { warningFocused = true }
    }

    private var card: some View {
        VStack(spacing: 0) {
            // read first as one element: the title and the warning
            VStack(spacing: 10) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .font(.title2)
                    .foregroundStyle(Theme.amber)
                    .accessibilityHidden(true)
                Text(model.t("flashTitle"))
                    .font(.title3.weight(.semibold))
                    .foregroundStyle(Theme.text)
                    .accessibilityAddTraits(.isHeader)
                Text(warning)
                    .font(.body)
                    .foregroundStyle(Theme.warnText)
            }
            .multilineTextAlignment(.center)
            .fixedSize(horizontal: false, vertical: true)
            .accessibilityElement(children: .combine)
            .accessibilityFocused($warningFocused)
            .accessibilitySortPriority(2)

            VStack(spacing: 10) {
                Button { model.dispatch(Ev.acceptFlash) } label: {
                    Text(model.t("flashPlay")).font(.body.weight(.semibold)).lineLimit(nil).multilineTextAlignment(.center)
                        .padding(.vertical, 10)
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(LumenButtonStyle(primary: true))
                Button { model.dispatch(Ev.closeSend) } label: {
                    Text(model.t("close")).font(.body.weight(.semibold)).lineLimit(nil).multilineTextAlignment(.center)
                        .padding(.vertical, 10)
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(LumenButtonStyle())
            }
            .padding(.top, 22)
            .accessibilitySortPriority(1)
        }
        .padding(24)
        .frame(maxWidth: 440)
        .panel(opaque: true, radius: 22)
        .overlay(RoundedRectangle(cornerRadius: 22, style: .continuous).stroke(Theme.amber.opacity(0.28), lineWidth: 1))
        .accessibilityElement(children: .contain)
        .accessibilityAddTraits(.isModal)
        .padding(20)
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

/// The system paste control (UIPasteControl): reading the pasteboard on its tap needs no
/// permission prompt. Unlike SwiftUI's PasteButton it takes its frame, so it gets a 44 pt
/// target, and the app's colours. Its label ("Paste" / "粘贴") follows the system language.
private struct PasteControl: UIViewRepresentable {
    let onPaste: (String) -> Void

    func makeUIView(context: Context) -> Host { Host() }

    func updateUIView(_ host: Host, context: Context) { host.onPaste = onPaste }

    func sizeThatFits(_ proposal: ProposedViewSize, uiView host: Host, context: Context) -> CGSize? {
        let w = host.control.intrinsicContentSize.width
        return CGSize(width: max(44, w.isFinite && w > 0 ? w + 8 : 76), height: 44)
    }

    final class Host: UIView {
        var onPaste: ((String) -> Void)?
        let control: UIPasteControl

        override init(frame: CGRect) {
            let config = UIPasteControl.Configuration()
            config.displayMode = .labelOnly
            config.cornerStyle = .capsule
            config.baseBackgroundColor = UIColor(Theme.cyan)
            config.baseForegroundColor = UIColor(Theme.ink)
            control = UIPasteControl(configuration: config)
            super.init(frame: frame)
            pasteConfiguration = UIPasteConfiguration(acceptableTypeIdentifiers: [UTType.text.identifier])
            control.target = self
            control.translatesAutoresizingMaskIntoConstraints = false
            addSubview(control)
            NSLayoutConstraint.activate([
                control.leadingAnchor.constraint(equalTo: leadingAnchor),
                control.trailingAnchor.constraint(equalTo: trailingAnchor),
                control.topAnchor.constraint(equalTo: topAnchor),
                control.bottomAnchor.constraint(equalTo: bottomAnchor),
            ])
        }

        required init?(coder: NSCoder) { fatalError() }

        override func paste(itemProviders: [NSItemProvider]) {
            guard let p = itemProviders.first(where: { $0.canLoadObject(ofClass: NSString.self) }) else { return }
            _ = p.loadObject(ofClass: NSString.self) { [weak self] obj, _ in
                guard let s = obj as? String else { return }
                DispatchQueue.main.async { self?.onPaste?(s) }
            }
        }
    }
}
