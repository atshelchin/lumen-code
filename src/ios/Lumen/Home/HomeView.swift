import SwiftUI

/// The landing screen: brand, hero, the two choices and the facts (the web's index.html).
struct HomeView: View {
    @Environment(AppModel.self) private var model
    let view: ViewModel

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                HStack {
                    HStack(spacing: 10) {
                        BrandMark()
                        Text("Lumen Code").font(.system(size: 16, weight: .semibold)).foregroundStyle(Theme.text)
                    }
                    Spacer()
                    Button { model.dispatch(Ev.toggleLang) } label: {
                        Text("中 / EN")
                            .font(.system(size: 12))
                            .foregroundStyle(Theme.muted)
                            .padding(.horizontal, 10).padding(.vertical, 5)
                            .overlay(RoundedRectangle(cornerRadius: 9).stroke(Theme.line, lineWidth: 1))
                    }
                    .buttonStyle(.plain)
                }
                .padding(.top, 8)

                VStack(alignment: .leading, spacing: 14) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(model.t("heroA")).foregroundStyle(Theme.text)
                        Text(model.t("heroB")).foregroundStyle(Theme.heroGradient)
                    }
                    .font(.system(size: 36, weight: .bold))
                    Text(model.t("heroP"))
                        .font(.system(size: 16))
                        .foregroundStyle(Theme.muted)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .tracking(-0.3)
                .padding(.top, 44)
                .padding(.bottom, 30)

                ViewThatFits(in: .horizontal) {
                    HStack(alignment: .top, spacing: 14) { cards }
                    VStack(spacing: 14) { cards }
                }

                LazyVGrid(columns: [GridItem(.adaptive(minimum: 150), spacing: 24, alignment: .topLeading)], alignment: .leading, spacing: 18) {
                    fact("f1t", "f1p")
                    fact("f2t", "f2p")
                    fact("f3t", "f3p")
                    fact("f4t", "f4p")
                }
                .padding(.top, 26)
                .overlay(alignment: .top) { Rectangle().fill(Theme.line).frame(height: 1) }
                .padding(.top, 36)

                Text(model.t("foot"))
                    .font(.system(size: 12))
                    .foregroundStyle(Theme.faint)
                    .padding(.top, 32)
                // the version, for reports: app version and the commit of the sources
                Text("\(model.t("buildLabel")) \(Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "") · \(model.view?.build ?? "")")
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(Theme.faint)
                    .textSelection(.enabled)
                    .padding(.top, 6)
                    .padding(.bottom, 12)
            }
            .padding(.horizontal, 20)
            .frame(maxWidth: 980)
            .frame(maxWidth: .infinity)
        }
        .scrollIndicators(.hidden)
    }

    @ViewBuilder private var cards: some View {
        ChoiceCard(symbol: "arrow.up.square", tint: Theme.cyan, title: model.t("sendTitle"), desc: model.t("sendDesc"), go: model.t("go")) {
            model.dispatch(Ev.navigate(.send))
        }
        ChoiceCard(symbol: "viewfinder", tint: Theme.pink, title: model.t("recvTitle"), desc: model.t("recvDesc"), go: model.t("go")) {
            model.dispatch(Ev.navigate(.receive))
        }
    }

    private func fact(_ t: String, _ p: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(model.t(t)).font(.system(size: 14, weight: .semibold)).foregroundStyle(Theme.text)
            Text(model.t(p)).font(.system(size: 13)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
        }
    }
}

private struct ChoiceCard: View {
    let symbol: String
    let tint: Color
    let title: String
    let desc: String
    let go: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 10) {
                IconTile(symbol: symbol, tint: tint, size: 46)
                Text(title).font(.system(size: 20, weight: .semibold)).foregroundStyle(Theme.text).padding(.top, 6)
                Text(desc).font(.system(size: 14)).foregroundStyle(Theme.muted).multilineTextAlignment(.leading).fixedSize(horizontal: false, vertical: true)
                Spacer(minLength: 0)
                Text(go).font(.system(size: 14, weight: .semibold)).foregroundStyle(Theme.text).padding(.top, 8)
            }
            .padding(22)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                RoundedRectangle(cornerRadius: 20, style: .continuous)
                    .fill(LinearGradient(colors: [Color.white.opacity(0.05), Color.white.opacity(0.02)], startPoint: .top, endPoint: .bottom))
            )
            .overlay(RoundedRectangle(cornerRadius: 20, style: .continuous).stroke(Theme.line, lineWidth: 1))
            .contentShape(RoundedRectangle(cornerRadius: 20))
        }
        .buttonStyle(CardPress())
    }
}

private struct CardPress: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(configuration.isPressed ? 0.98 : 1)
            .animation(.spring(duration: 0.2), value: configuration.isPressed)
    }
}
