import LumenCore
import SwiftUI
import UIKit

/// The result: text and images fill the screen (reader); other files get a card.
struct DoneScreen: View {
    @Environment(AppModel.self) private var model
    let done: DoneVM

    var body: some View {
        if done.reader {
            reader
        } else {
            card
        }
    }

    private var badge: some View {
        Image(systemName: "checkmark")
            .font(.system(size: done.reader ? 16 : 26, weight: .bold))
            .foregroundStyle(Theme.green)
            .frame(width: done.reader ? 36 : 58, height: done.reader ? 36 : 58)
            .background(Circle().fill(Theme.green.opacity(0.14)))
    }

    private var stats: some View {
        VStack(alignment: .leading, spacing: 10) {
            LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 8, alignment: .topLeading), count: 3), alignment: .leading, spacing: 10) {
                ForEach(done.stats, id: \.self) { m in
                    VStack(alignment: .leading, spacing: 1) {
                        Text(m.label).font(.system(size: 11)).foregroundStyle(Theme.faint).lineLimit(1)
                        Text(m.value).font(Theme.mono(13.5, .semibold)).foregroundStyle(Theme.text).lineLimit(1).minimumScaleFactor(0.75)
                    }
                }
            }
            if let code = done.codeLine {
                Text(code).font(.system(size: 11.5)).foregroundStyle(Theme.faint).fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(.vertical, 12)
        .padding(.horizontal, 10)
        .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Color.white.opacity(0.03)))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(Theme.line, lineWidth: 1))
    }

    private var actions: some View {
        let text = done.copyLabel != nil
        return FlowRow(spacing: 10) {
            if let copy = done.copyLabel {
                Button(copy) { model.dispatch(Ev.copy) }.buttonStyle(LumenButtonStyle(primary: true))
            }
            if done.link != nil {
                Button(model.t("openLink")) { model.dispatch(Ev.openLink) }.buttonStyle(LumenButtonStyle())
            }
            Button(done.saveLabel) { model.dispatch(Ev.save) }.buttonStyle(LumenButtonStyle(primary: done.savePrimary && !text))
            Button(model.t("share")) { model.dispatch(Ev.share) }.buttonStyle(LumenButtonStyle())
            if done.canOpen && !text && done.kind != "image" {
                Button(model.t("open")) { model.dispatch(Ev.openFile) }.buttonStyle(LumenButtonStyle())
            }
            Button(model.t("again")) { model.dispatch(Ev.receiveAnother) }.buttonStyle(LumenButtonStyle())
        }
    }

    private var note: some View {
        Group {
            if let n = done.note {
                Text(n).font(.system(size: 12.5)).foregroundStyle(done.noteOk ? Theme.green : Theme.red)
            }
        }
    }

    // MARK: card

    private var card: some View {
        ZStack {
            Color(red: 4 / 255, green: 6 / 255, blue: 10 / 255).opacity(0.85).ignoresSafeArea()
            ScrollView {
                VStack(spacing: 0) {
                    badge.padding(.bottom, 14)
                    Text(done.title).font(.system(size: 19, weight: .semibold)).foregroundStyle(Theme.text).multilineTextAlignment(.center).padding(.bottom, 4)
                    Text(done.sub).font(.system(size: 13.5)).foregroundStyle(Theme.muted).multilineTextAlignment(.center)
                    stats.padding(.top, 16)
                    actions.padding(.top, 18)
                    note.padding(.top, 10)
                }
                .padding(24)
                .frame(maxWidth: 460)
                .panel()
                .padding(14)
                .frame(maxWidth: .infinity, minHeight: UIScreen.main.bounds.height * 0.8)
            }
            .scrollIndicators(.hidden)
        }
    }

    // MARK: reader

    private var reader: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                badge
                VStack(alignment: .leading, spacing: 1) {
                    Text(done.title).font(.system(size: 16, weight: .semibold)).foregroundStyle(Theme.text).lineLimit(1)
                    Text(done.sub).font(.system(size: 12.5)).foregroundStyle(Theme.muted).lineLimit(1)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                Button { model.dispatch(Ev.toggleStats) } label: {
                    HStack(spacing: 4) {
                        Text(model.t("details")).font(.system(size: 12.5))
                        Image(systemName: "chevron.down").font(.system(size: 11, weight: .semibold)).rotationEffect(.degrees(done.statsOpen ? 180 : 0))
                    }
                    .foregroundStyle(Theme.muted)
                    .padding(.leading, 12).padding(.trailing, 9)
                    .frame(minHeight: 34)
                    .background(Capsule().fill(Color.white.opacity(0.04)))
                    .overlay(Capsule().stroke(Theme.line2, lineWidth: 1))
                }
                .buttonStyle(.plain)
            }
            .padding(.horizontal, 16)
            .padding(.top, 4)
            .padding(.bottom, 10)
            if done.statsOpen {
                stats.padding(.horizontal, 16).padding(.bottom, 10)
            }
            if done.kind != "image" {
                HStack(spacing: 6) {
                    if done.json {
                        Segmented(items: [(true, model.t("vPretty")), (false, model.t("vRaw"))], selected: done.pretty) { model.dispatch(Ev.jsonView(pretty: $0)) }
                    }
                    Spacer()
                    Button { model.dispatch(Ev.font(-1)) } label: { Text("A").font(.system(size: 12, weight: .bold)) }
                        .buttonStyle(ToolButtonStyle()).accessibilityLabel(model.t("smaller"))
                    Button { model.dispatch(Ev.font(1)) } label: { Text("A").font(.system(size: 18, weight: .bold)) }
                        .buttonStyle(ToolButtonStyle()).accessibilityLabel(model.t("larger"))
                    if done.kind != "message" {
                        Button { model.dispatch(Ev.toggleWrap) } label: { Image(systemName: "text.word.spacing") }
                            .buttonStyle(ToolButtonStyle(pressed: done.wrap))
                            .accessibilityLabel(model.t("wrap"))
                    }
                }
                .padding(.horizontal, 16)
                .padding(.bottom, 8)
            }
            Group {
                if done.kind == "image" {
                    ZoomImage(core: model.core, rev: done.contentRev)
                        .background(Color.black)
                } else {
                    ReaderText(core: model.core, done: done, onLink: { url in UIApplication.shared.open(url) })
                        .background(Color.black.opacity(0.22))
                }
            }
            .overlay(alignment: .top) { Rectangle().fill(Theme.line).frame(height: 1) }
            VStack(spacing: 6) {
                actions
                note
            }
            .padding(.horizontal, 16)
            .padding(.top, 10)
            .padding(.bottom, 6)
        }
        .background(Theme.bg.ignoresSafeArea())
    }
}

/// Received text in a UITextView: fast for megabytes, selectable, links tappable.
private struct ReaderText: UIViewRepresentable {
    let core: LumenCore
    let done: DoneVM
    let onLink: (URL) -> Void

    final class Coordinator: NSObject, UITextViewDelegate {
        var key = ""
        var onLink: ((URL) -> Void)?
        func textView(_ textView: UITextView, primaryActionFor textItem: UITextItem, defaultAction: UIAction) -> UIAction? {
            if case .link(let url) = textItem.content {
                return UIAction { [weak self] _ in self?.onLink?(url) }
            }
            return defaultAction
        }
    }

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeUIView(context: Context) -> UITextView {
        let tv = UITextView()
        tv.isEditable = false
        tv.isSelectable = true
        tv.backgroundColor = .clear
        tv.alwaysBounceVertical = true
        tv.indicatorStyle = .white
        tv.delegate = context.coordinator
        tv.linkTextAttributes = [.foregroundColor: UIColor(red: 34 / 255, green: 216 / 255, blue: 1, alpha: 1),
                                 .underlineStyle: NSUnderlineStyle.single.rawValue,
                                 .underlineColor: UIColor(red: 34 / 255, green: 216 / 255, blue: 1, alpha: 0.4)]
        return tv
    }

    func updateUIView(_ tv: UITextView, context: Context) {
        context.coordinator.onLink = onLink
        let plain = done.kind == "message"
        let key = "\(done.contentRev)|\(done.pretty)|\(done.font)|\(done.wrap)"
        guard key != context.coordinator.key else { return }
        context.coordinator.key = key
        guard let content = core.reader(pretty: done.pretty) else { return }
        let (text, width) = ReaderFormat.attributed(content, plain: plain, json: done.json && done.pretty, fontStep: done.font, wrap: done.wrap || plain)
        let wrap = done.wrap || plain
        tv.textContainer.widthTracksTextView = wrap
        tv.textContainer.lineBreakMode = wrap ? .byWordWrapping : .byClipping
        if !wrap { tv.textContainer.size = CGSize(width: width, height: .greatestFiniteMagnitude) }
        tv.textContainerInset = plain ? UIEdgeInsets(top: 22, left: 16, bottom: 64, right: 16) : UIEdgeInsets(top: 14, left: 12, bottom: 48, right: 12)
        tv.attributedText = text
        tv.setContentOffset(.zero, animated: false)
    }
}

enum ReaderFormat {
    static let colors: [JsonSpanKind: UIColor] = [
        .key: UIColor(red: 0x79 / 255, green: 0xd3 / 255, blue: 1, alpha: 1),
        .str: UIColor(red: 0x9f / 255, green: 0xe3 / 255, blue: 0x9a / 255, alpha: 1),
        .num: UIColor(red: 1, green: 0xcf / 255, blue: 0x5c / 255, alpha: 1),
        .bool: UIColor(red: 1, green: 0x7a / 255, blue: 0xb8 / 255, alpha: 1),
        .null: UIColor(red: 0xb9 / 255, green: 0xa4 / 255, blue: 1, alpha: 1),
        .punct: UIColor(red: 0x6b / 255, green: 0x74 / 255, blue: 0x88 / 255, alpha: 1),
    ]
    static let text = UIColor(red: 0xe9 / 255, green: 0xed / 255, blue: 0xf4 / 255, alpha: 1)
    static let faint = UIColor(red: 0x5b / 255, green: 0x64 / 255, blue: 0x76 / 255, alpha: 0.6)

    /// The attributed text and, for unwrapped text, the width it needs.
    static func attributed(_ c: ReaderContent, plain: Bool, json: Bool, fontStep: Int, wrap: Bool) -> (NSAttributedString, CGFloat) {
        if json && !c.lines.isEmpty {
            return jsonText(c.lines, size: CGFloat(13 + fontStep))
        }
        let s = c.text ?? ""
        if plain {
            let size = CGFloat(17 + fontStep)
            let para = NSMutableParagraphStyle()
            para.lineSpacing = size * 0.45
            let out = NSMutableAttributedString(string: s, attributes: [.font: UIFont.systemFont(ofSize: size), .foregroundColor: text, .paragraphStyle: para])
            let ns = s as NSString
            for l in c.links {
                let r = NSRange(location: Int(l.start), length: Int(l.end - l.start))
                if r.location + r.length <= ns.length, let url = URL(string: ns.substring(with: r)) {
                    out.addAttribute(.link, value: url, range: r)
                }
            }
            return (out, 0)
        }
        let size = CGFloat(13 + fontStep)
        let font = UIFont.monospacedSystemFont(ofSize: size, weight: .regular)
        let para = NSMutableParagraphStyle()
        para.lineSpacing = size * 0.6
        let longest = s.split(separator: "\n", omittingEmptySubsequences: false).reduce(0) { max($0, $1.count) }
        let ch = ("0" as NSString).size(withAttributes: [.font: font]).width
        return (NSAttributedString(string: s, attributes: [.font: font, .foregroundColor: text, .paragraphStyle: para]), CGFloat(longest) * ch + 48)
    }

    /// Formatted JSON: line numbers in a gutter, indentation by depth, wrapped lines hanging
    /// two levels further in (the web reader's layout).
    private static func jsonText(_ lines: [JsonLine], size: CGFloat) -> (NSAttributedString, CGFloat) {
        let font = UIFont.monospacedSystemFont(ofSize: size, weight: .regular)
        let ch = ("0" as NSString).size(withAttributes: [.font: font]).width
        let gut = CGFloat(String(lines.count).count + 2) * ch
        let out = NSMutableAttributedString()
        var widest: CGFloat = 0
        var paras: [Int: NSParagraphStyle] = [:]
        for (i, line) in lines.enumerated() {
            let d = Int(line.depth)
            let style: NSParagraphStyle
            if let s = paras[d] {
                style = s
            } else {
                let p = NSMutableParagraphStyle()
                p.lineSpacing = size * 0.6
                p.tabStops = [NSTextTab(textAlignment: .right, location: gut - 1.5 * ch), NSTextTab(textAlignment: .left, location: gut + CGFloat(d) * 2 * ch)]
                p.headIndent = gut + CGFloat(d + 2) * 2 * ch
                style = p
                paras[d] = p
            }
            let start = out.length
            out.append(NSAttributedString(string: "\t\(i + 1)\t", attributes: [.font: font, .foregroundColor: faint]))
            var chars = 0
            for sp in line.spans {
                out.append(NSAttributedString(string: sp.text, attributes: [.font: font, .foregroundColor: colors[sp.kind] ?? text]))
                chars += sp.text.count
            }
            if i < lines.count - 1 { out.append(NSAttributedString(string: "\n")) }
            out.addAttribute(.paragraphStyle, value: style, range: NSRange(location: start, length: out.length - start))
            widest = max(widest, gut + CGFloat(d * 2 + chars) * ch)
        }
        return (out, widest + 48)
    }
}

/// A received image, pinch to zoom.
private struct ZoomImage: UIViewRepresentable {
    let core: LumenCore
    let rev: UInt64

    final class Box: UIScrollView, UIScrollViewDelegate {
        let imageView = UIImageView()
        var rev: UInt64 = .max
        override init(frame: CGRect) {
            super.init(frame: frame)
            delegate = self
            minimumZoomScale = 1
            maximumZoomScale = 8
            showsVerticalScrollIndicator = false
            showsHorizontalScrollIndicator = false
            imageView.contentMode = .scaleAspectFit
            addSubview(imageView)
            let dbl = UITapGestureRecognizer(target: self, action: #selector(double))
            dbl.numberOfTapsRequired = 2
            addGestureRecognizer(dbl)
        }
        required init?(coder: NSCoder) { fatalError() }
        override func layoutSubviews() {
            super.layoutSubviews()
            if zoomScale == 1 { imageView.frame = bounds }
        }
        func viewForZooming(in scrollView: UIScrollView) -> UIView? { imageView }
        @objc func double() { setZoomScale(zoomScale > 1 ? 1 : 2.5, animated: true) }
    }

    func makeUIView(context: Context) -> Box { Box() }

    func updateUIView(_ v: Box, context: Context) {
        guard v.rev != rev else { return }
        v.rev = rev
        if let data = core.receivedBytes() { v.imageView.image = UIImage(data: data) }
    }
}
