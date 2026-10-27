import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// The sender (the web's send page): pick a file or type text; then the code fills the space
/// above a slim permanent bar. Nothing is ever drawn over the code (a covered corner stops the
/// receiver): the settings open as a panel above the bar, or beside the code on a phone held
/// sideways, and the code shrinks to the space that is left.
struct SendScreen: View {
    @Environment(AppModel.self) private var model
    let view: ViewModel

    /// The bar's height above the bottom safe area (a row of 44 pt targets).
    static let barHeight: CGFloat = 56

    /// A phone on its side: the settings open beside the code, which keeps its height.
    static func sideSettings(_ size: CGSize) -> Bool {
        size.width > size.height && size.height < 520
    }

    var body: some View {
        let s = view.send
        GeometryReader { outer in
            let side = Self.sideSettings(outer.size)
            ZStack {
                if s.phase == "pick" {
                    PickCard(send: s)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else {
                    Color(hex: 0x0a0d14).ignoresSafeArea()
                    VStack(spacing: 0) {
                        HStack(spacing: 0) {
                            CodeArea(send: s)
                            if s.settingsOpen && side {
                                SettingsPanel(send: s, side: true, labelWidth: 84, maxHeight: .infinity)
                                    .frame(width: min(360, outer.size.width * 0.46))
                                    .background(alignment: .leading) {
                                        ZStack(alignment: .leading) {
                                            Theme.panelSolid
                                            Theme.line.frame(width: 1)
                                        }
                                        .ignoresSafeArea(edges: [.top, .trailing])
                                    }
                                    .debugFrame("settings")
                                    .transition(.move(edge: .trailing))
                            }
                        }
                        if s.settingsOpen && !side {
                            SettingsPanel(send: s, side: false, labelWidth: outer.size.width < 420 ? 84 : 112, maxHeight: outer.size.height * 0.5)
                                .background(alignment: .top) { barBackground }
                                .debugFrame("settings")
                                .transition(.move(edge: .bottom).combined(with: .opacity))
                        }
                        SendBar(send: s)
                            .frame(height: Self.barHeight)
                            .background(alignment: .top) { barBackground }
                            .debugFrame("bar")
                    }
                    // under the warning the bar stays in sight but out of reach (its card has Close)
                    .accessibilityHidden(s.flashAsk)
                    .allowsHitTesting(!s.flashAsk)
                    if s.flashAsk {
                        FlashAsk(warning: s.flashWarn)
                            .transition(.opacity)
                    }
                }
            }
            .animation(.easeOut(duration: 0.2), value: s.flashAsk)
            .animation(.easeOut(duration: 0.2), value: s.settingsOpen)
            // before a code exists, the space it will get (settings closed): the first grid fits
            .onAppear { if s.phase == "pick" { predict(outer.size, settings: s.settingsOpen) } }
            .onChange(of: outer.size) { _, size in if view.send.phase == "pick" { predict(size, settings: view.send.settingsOpen) } }
        }
        .overlay(alignment: .topLeading) {
            if s.phase == "pick" {
                HomeButton { model.dispatch(Ev.navigate(.home)) }
                    .padding(.horizontal, 14)
                    .padding(.top, 8)
            }
        }
    }

    /// The bar's and the panel's solid background with its top line, out to the screen's edges.
    private var barBackground: some View {
        ZStack(alignment: .top) {
            Theme.panelSolid
            Theme.line.frame(height: 1)
        }
        .ignoresSafeArea(edges: [.horizontal, .bottom])
    }

    private func predict(_ size: CGSize, settings: Bool) {
        guard !settings else { return } // the panel's height is not known yet; the code area reports it
        SendScreen.report(model, CGSize(width: size.width, height: size.height - Self.barHeight))
    }

    /// The code's box (the space above the bar and the settings) goes to the core, which turns
    /// it into a grid with its quiet zone inside the box.
    static func report(_ model: AppModel, _ size: CGSize) {
        guard size.width > 0, size.height > 0 else { return }
        let scale = UIScreen.main.scale
        let pad = UIDevice.current.userInterfaceIdiom == .pad
        // physical size of a point: ~0.16 mm on iPhones, ~0.19 mm on iPads
        let mmPerPt = pad ? 0.19 : 0.16
        model.dispatch(Ev.viewport(width: Double(size.width * scale), height: Double(size.height * scale),
                                   scale: Double(scale), mmPerPx: mmPerPt / Double(scale)))
    }
}

/// Where the code is drawn: whatever the bar and the settings leave.
private struct CodeArea: View {
    @Environment(AppModel.self) private var model
    let send: SendVM

    var body: some View {
        GeometryReader { p in
            ZStack {
                if let g = send.geometry {
                    CodeView(model: model, geometry: g, frameRev: send.frameRev, playing: send.playing, fps: send.fps)
                        .accessibilityHidden(send.flashAsk)
                }
            }
            .frame(width: p.size.width, height: p.size.height)
            .onAppear { SendScreen.report(model, p.size) }
            .onChange(of: p.size) { _, size in SendScreen.report(model, size) }
        }
        .clipped()
        .debugFrame("codeArea")
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
            // dims the paused code and the bar, and keeps their taps away
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

/// The permanent bar: play or pause, what is sent (and how long a pass takes), settings, close.
private struct SendBar: View {
    @Environment(AppModel.self) private var model
    let send: SendVM

    var body: some View {
        let paused = !send.playing && send.phase == "ready"
        HStack(spacing: 4) {
            Button {
                model.dispatch(Ev.togglePlay)
                Haptics.play("light")
            } label: {
                Image(systemName: send.playing ? "pause.fill" : "play.fill")
                    .font(.system(size: 17, weight: .semibold))
            }
            .buttonStyle(PlayButtonStyle())
            .disabled(send.phase != "ready")
            .accessibilityLabel(model.t(send.playing ? "pause" : "play"))
            .accessibilityIdentifier("play")

            VStack(alignment: .leading, spacing: 1) {
                FileName(name: send.title)
                Text(send.subtitle)
                    .font(.system(size: 12.5))
                    .monospacedDigit()
                    .foregroundStyle(paused ? Theme.amber : Theme.muted)
                    .lineLimit(1)
            }
            .padding(.horizontal, 6)
            .frame(maxWidth: .infinity, alignment: .leading)
            .accessibilityElement(children: .combine)

            Button { model.dispatch(Ev.toggleSettings) } label: { Image(systemName: "slider.horizontal.3") }
                .buttonStyle(BarButtonStyle(active: send.settingsOpen))
                .accessibilityLabel(model.t("settings"))
                .accessibilityAddTraits(send.settingsOpen ? .isSelected : [])
            Button { model.dispatch(Ev.closeSend) } label: { Image(systemName: "xmark") }
                .buttonStyle(BarButtonStyle())
                .accessibilityLabel(model.t("close"))
        }
        .padding(.horizontal, 10)
    }
}

/// A file name cut in the middle, keeping the end of the stem and the extension in sight.
private struct FileName: View {
    let name: String

    var body: some View {
        let (head, tail) = Self.split(name)
        HStack(spacing: 0) {
            Text(head).lineLimit(1).truncationMode(.tail)
            Text(tail).lineLimit(1).fixedSize()
        }
        .font(.system(size: 14, weight: .semibold))
        .foregroundStyle(Theme.text)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(name)
    }

    static func split(_ n: String) -> (String, String) {
        let chars = Array(n)
        var ext = ""
        if let dot = chars.lastIndex(of: "."), dot > 0, chars.count - dot <= 8 { ext = String(chars[dot...]) }
        let stem = Array(chars.prefix(chars.count - ext.count))
        let keep = min(stem.count, 4)
        return (String(stem.prefix(stem.count - keep)), String(stem.suffix(keep)) + ext)
    }
}

private struct PlayButtonStyle: ButtonStyle {
    @Environment(\.isEnabled) private var enabled
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .foregroundStyle(Theme.ink)
            .frame(width: 44, height: 44)
            .background(Circle().fill(configuration.isPressed ? Color.white : Theme.paper))
            .scaleEffect(configuration.isPressed ? 0.95 : 1)
            .opacity(enabled ? 1 : 0.5)
            .contentShape(Circle())
    }
}

/// The bar's quiet buttons: no frame, a light fill when pressed or selected.
private struct BarButtonStyle: ButtonStyle {
    var active = false
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 18, weight: .medium))
            .foregroundStyle(active || configuration.isPressed ? Theme.text : Theme.muted)
            .frame(width: 44, height: 44)
            .background(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(Color.white.opacity(active ? 0.12 : (configuration.isPressed ? 0.06 : 0)))
            )
            .contentShape(Rectangle())
    }
}

/// Density, colours, error correction, frame rate, and the details line.
private struct SettingsPanel: View {
    @Environment(AppModel.self) private var model
    let send: SendVM
    let side: Bool
    let labelWidth: CGFloat
    let maxHeight: CGFloat
    @State private var fps: Double = 12
    @State private var editing = false
    @State private var contentHeight: CGFloat = 0

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 10) {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 330), spacing: 24, alignment: .leading)], alignment: .leading, spacing: 8) {
                    field("density") {
                        Segmented(items: [("auto", model.t("dAuto")), ("low", model.t("dLow")), ("high", model.t("dHigh")), ("max", model.t("dMax"))],
                                  selected: send.density, fill: true, height: 36) {
                            model.dispatch(Ev.density($0))
                            Haptics.play("selection")
                        }
                    }
                    field("colors") {
                        Segmented(items: [(1, "4"), (2, "8")], selected: Int(send.mode), fill: true, height: 36) {
                            model.dispatch(Ev.mode($0))
                            Haptics.play("selection")
                        }
                    }
                    field("ecc") {
                        Segmented(items: [(0, model.t("eLow")), (1, model.t("eMid")), (2, model.t("eHigh"))], selected: Int(send.ecc), fill: true, height: 36) {
                            model.dispatch(Ev.ecc($0))
                            Haptics.play("selection")
                        }
                    }
                    field("speed") {
                        HStack(spacing: 10) {
                            Slider(value: $fps, in: 4...60, step: 1) { editing = $0 }
                                .tint(Theme.cyan)
                                .frame(minHeight: 44)
                                .accessibilityLabel(model.t("speed"))
                                .accessibilityValue("\(Int(fps))")
                            Text("\(Int(fps))").font(Theme.mono(13, .bold)).foregroundStyle(Theme.text)
                                .frame(minWidth: 24, alignment: .trailing)
                                .accessibilityHidden(true)
                        }
                    }
                }
                if !send.details.isEmpty {
                    (Text(model.t("details")).fontWeight(.semibold).foregroundStyle(Theme.muted) + Text("  " + send.details).foregroundStyle(Theme.faint))
                        .font(.system(size: 11.5))
                        .monospacedDigit()
                        .lineSpacing(3)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .padding(.horizontal, 10)
            .padding(.top, 12)
            .padding(.bottom, 10)
            .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { contentHeight = $0 }
        }
        .scrollBounceBehavior(.basedOnSize)
        .scrollIndicators(.hidden)
        .frame(height: side ? nil : min(max(contentHeight, 1), maxHeight))
        .onAppear { fps = Double(send.fps) }
        .onChange(of: send.fps) { _, v in if !editing && Int(fps) != Int(v) { fps = Double(v) } }
        .onChange(of: fps) { _, v in if Int(v) != Int(send.fps) { model.dispatch(Ev.fps(Int(v))) } }
    }

    private func field<C: View>(_ key: String, @ViewBuilder _ content: () -> C) -> some View {
        HStack(spacing: 10) {
            Text(model.t(key))
                .font(.system(size: 13))
                .foregroundStyle(Theme.muted)
                .fixedSize(horizontal: false, vertical: true)
                .frame(width: labelWidth, alignment: .leading)
                .accessibilityHidden(true)
            content()
                .frame(maxWidth: .infinity)
                .accessibilityElement(children: .contain)
                .accessibilityLabel(model.t(key))
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
