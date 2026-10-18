import SwiftUI

/// Design tokens of the web demo (`src/web/css/app.css`), one to one.
enum Theme {
    static let bg = Color(hex: 0x07090d)
    static let bg2 = Color(hex: 0x0c1017)
    static let ink = Color(hex: 0x0a0d14)
    static let panel = Color(red: 17 / 255, green: 21 / 255, blue: 30 / 255).opacity(0.78)
    static let panelSolid = Color(hex: 0x11151e)
    /// Panels over live video or a playing code: no blur (heat), a little more opaque.
    static let panelOpaque = Color(red: 17 / 255, green: 21 / 255, blue: 30 / 255).opacity(0.95)
    static let line = Color.white.opacity(0.08)
    static let line2 = Color.white.opacity(0.14)
    static let text = Color(hex: 0xe9edf4)
    static let muted = Color(hex: 0x8b94a7)
    static let faint = Color(hex: 0x5b6476)
    static let cyan = Color(hex: 0x22d8ff)
    static let pink = Color(hex: 0xff3d9a)
    static let amber = Color(hex: 0xffd60a)
    static let green = Color(hex: 0x2bdc74)
    static let red = Color(hex: 0xff5468)
    static let paper = Color(hex: 0xf2f4f8)
    static let markDark = Color(hex: 0x1b2130)

    static let radius: CGFloat = 16
    static let radiusSm: CGFloat = 10

    static let spectrum = LinearGradient(colors: [cyan, pink, amber], startPoint: .leading, endPoint: .trailing)
    static let heroGradient = LinearGradient(stops: [.init(color: cyan, location: 0), .init(color: pink, location: 0.55), .init(color: amber, location: 1)], startPoint: .leading, endPoint: .trailing)

    static func mono(_ size: CGFloat, _ weight: Font.Weight = .regular) -> Font {
        .system(size: size, weight: weight, design: .monospaced)
    }
}

extension Color {
    init(hex: UInt32, opacity: Double = 1) {
        self.init(.sRGB, red: Double((hex >> 16) & 0xff) / 255, green: Double((hex >> 8) & 0xff) / 255, blue: Double(hex & 0xff) / 255, opacity: opacity)
    }
}

/// The brand mark: a tiny 3×3 code.
struct BrandMark: View {
    var cell: CGFloat = 7
    private let colors: [Color] = [Theme.cyan, Theme.markDark, Theme.pink, Theme.markDark, Theme.paper, Theme.amber, Theme.markDark, Theme.cyan, Theme.green]
    var body: some View {
        let gap = cell * 3 / 7
        VStack(spacing: gap) {
            ForEach(0..<3) { r in
                HStack(spacing: gap) {
                    ForEach(0..<3) { c in
                        RoundedRectangle(cornerRadius: cell * 2 / 7, style: .continuous).fill(colors[r * 3 + c]).frame(width: cell, height: cell)
                    }
                }
            }
        }
        .padding(cell * 6 / 7)
        .background(RoundedRectangle(cornerRadius: cell * 9 / 7, style: .continuous).fill(Theme.ink))
        .overlay(RoundedRectangle(cornerRadius: cell * 9 / 7, style: .continuous).stroke(Theme.line2, lineWidth: 1))
    }
}

// MARK: - buttons

struct LumenButtonStyle: ButtonStyle {
    var primary = false
    var compact = false
    @Environment(\.isEnabled) private var enabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 15, weight: .semibold))
            .lineLimit(1)
            .padding(.horizontal, compact ? 14 : 18)
            .frame(minHeight: 44)
            .foregroundStyle(primary ? Theme.ink : Theme.text)
            .background(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(primary ? (configuration.isPressed ? Color.white : Theme.paper) : Color.white.opacity(configuration.isPressed ? 0.1 : 0.04))
            )
            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(primary ? Color.clear : Theme.line2, lineWidth: 1))
            .scaleEffect(configuration.isPressed ? 0.98 : 1)
            .opacity(enabled ? 1 : 0.4)
            .animation(.easeOut(duration: 0.1), value: configuration.isPressed)
    }
}

struct IconButtonStyle: ButtonStyle {
    var active = false
    var size: CGFloat = 44
    var background: Color = Color.white.opacity(0.04)
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 18, weight: .medium))
            .foregroundStyle(Theme.text)
            .frame(width: size, height: size)
            .background(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(active ? Color.white.opacity(0.12) : (configuration.isPressed ? Color.white.opacity(0.1) : background))
            )
            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(Theme.line2, lineWidth: 1))
            .scaleEffect(configuration.isPressed ? 0.96 : 1)
    }
}

/// The web's `.tbtn`: small square tool buttons in the reader.
struct ToolButtonStyle: ButtonStyle {
    var pressed = false
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .foregroundStyle(pressed ? Theme.text : Theme.muted)
            .frame(minWidth: 36, minHeight: 34)
            .padding(.horizontal, 6)
            .background(RoundedRectangle(cornerRadius: 9, style: .continuous).fill(pressed ? Color.white.opacity(0.12) : Color.white.opacity(configuration.isPressed ? 0.1 : 0.04)))
            .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(pressed ? Theme.line2 : Theme.line, lineWidth: 1))
    }
}

// MARK: - segmented control

struct Segmented<T: Hashable>: View {
    let items: [(T, String)]
    let selected: T
    var minWidth: CGFloat = 0
    let pick: (T) -> Void

    var body: some View {
        HStack(spacing: 2) {
            ForEach(items, id: \.0) { item in
                let on = item.0 == selected
                Button {
                    if !on { pick(item.0) }
                } label: {
                    Text(item.1)
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(on ? Theme.text : Theme.muted)
                        .padding(.horizontal, 11)
                        .frame(minWidth: minWidth, minHeight: 32)
                        .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(on ? Color.white.opacity(0.12) : Color.clear))
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        .padding(3)
        .background(RoundedRectangle(cornerRadius: 11, style: .continuous).fill(Color.white.opacity(0.05)))
        .overlay(RoundedRectangle(cornerRadius: 11, style: .continuous).stroke(Theme.line, lineWidth: 1))
    }
}

// MARK: - panels, chips, dots

struct PanelBackground: ViewModifier {
    var opaque = false
    var radius: CGFloat = Theme.radius
    func body(content: Content) -> some View {
        content
            .background(
                RoundedRectangle(cornerRadius: radius, style: .continuous)
                    .fill(opaque ? Theme.panelOpaque : Theme.panel)
                    .background(opaque ? nil : RoundedRectangle(cornerRadius: radius, style: .continuous).fill(.ultraThinMaterial).environment(\.colorScheme, .dark))
            )
            .overlay(RoundedRectangle(cornerRadius: radius, style: .continuous).stroke(Theme.line, lineWidth: 1))
            .shadow(color: .black.opacity(0.45), radius: 20, y: 10)
    }
}

extension View {
    func panel(opaque: Bool = false, radius: CGFloat = Theme.radius) -> some View {
        modifier(PanelBackground(opaque: opaque, radius: radius))
    }
}

struct StatusDot: View {
    var live: Bool
    var warn = false
    var body: some View {
        Circle()
            .fill(live ? Theme.green : (warn ? Theme.amber : Theme.faint))
            .frame(width: 8, height: 8)
            .background(Circle().fill(live ? Theme.green.opacity(0.15) : .clear).frame(width: 16, height: 16))
    }
}

/// Rounded tinted square holding an icon (the web's `.drop-art` / `.choice .ico`).
struct IconTile: View {
    let symbol: String
    var tint: Color = Theme.cyan
    var size: CGFloat = 64
    var body: some View {
        Image(systemName: symbol)
            .font(.system(size: size * 0.44, weight: .medium))
            .foregroundStyle(tint)
            .frame(width: size, height: size)
            .background(RoundedRectangle(cornerRadius: size * 0.28, style: .continuous).fill(tint.opacity(0.11)))
    }
}

/// "‹ mark" — back to the home screen.
struct HomeButton: View {
    let action: () -> Void
    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Image(systemName: "chevron.left").font(.system(size: 15, weight: .bold))
                BrandMark(cell: 6)
            }
            .foregroundStyle(Theme.text)
            .padding(.leading, 4)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

/// Lays children out in rows that wrap (the web's flex-wrap rows of buttons).
struct FlowRow: Layout {
    var spacing: CGFloat = 10
    var alignCenter = true

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? .infinity
        let rows = arrange(width: width, subviews: subviews)
        let h = rows.reduce(0) { $0 + $1.height } + spacing * CGFloat(max(0, rows.count - 1))
        let w = rows.map(\.width).max() ?? 0
        return CGSize(width: proposal.width ?? w, height: h)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var y = bounds.minY
        for row in arrange(width: bounds.width, subviews: subviews) {
            var x = alignCenter ? bounds.minX + (bounds.width - row.width) / 2 : bounds.minX
            for i in row.items {
                let s = subviews[i].sizeThatFits(.unspecified)
                subviews[i].place(at: CGPoint(x: x, y: y + (row.height - s.height) / 2), proposal: ProposedViewSize(s))
                x += s.width + spacing
            }
            y += row.height + spacing
        }
    }

    private struct Row { var items: [Int] = []; var width: CGFloat = 0; var height: CGFloat = 0 }

    private func arrange(width: CGFloat, subviews: Subviews) -> [Row] {
        var rows: [Row] = []
        var cur = Row()
        for (i, v) in subviews.enumerated() {
            let s = v.sizeThatFits(.unspecified)
            let add = cur.items.isEmpty ? s.width : cur.width + spacing + s.width
            if add > width && !cur.items.isEmpty {
                rows.append(cur)
                cur = Row()
            }
            cur.width = cur.items.isEmpty ? s.width : cur.width + spacing + s.width
            cur.height = max(cur.height, s.height)
            cur.items.append(i)
        }
        if !cur.items.isEmpty { rows.append(cur) }
        return rows
    }
}
