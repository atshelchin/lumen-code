import PhotosUI
import SwiftUI
import UniformTypeIdentifiers

@main
struct LumenApp: App {
    @State private var model = AppModel()
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(model)
                .preferredColorScheme(.dark)
                .onOpenURL { model.open(url: $0) }
                .onChange(of: scenePhase) { _, phase in
                    if phase == .active { model.scenePhase(true) } else if phase == .background { model.scenePhase(false) }
                }
        }
    }
}

struct RootView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        @Bindable var model = model
        ZStack {
            Theme.bg.ignoresSafeArea()
            if let v = model.view {
                switch v.screen {
                case .home: HomeView(view: v).transition(.opacity)
                case .send: SendScreen(view: v).transition(.opacity)
                case .receive: ReceiveScreen(view: v).transition(.opacity)
                }
            }
        }
        .animation(.easeOut(duration: 0.18), value: model.view?.screen)
        .statusBarHidden(model.immersive)
        .persistentSystemOverlays(model.immersive ? .hidden : .automatic)
        .fileImporter(isPresented: $model.showFileImporter, allowedContentTypes: [.item], allowsMultipleSelection: false) { result in
            model.fileImported(result.flatMap { urls in urls.first.map { .success($0) } ?? .failure(CocoaError(.fileNoSuchFile)) })
        } onCancellation: {
            model.fileImportCancelled()
        }
        .photosPicker(isPresented: $model.showPhotoPicker, selection: $model.photoItem, matching: .images)
        .onChange(of: model.showPhotoPicker) { _, shown in
            if !shown { model.photoPicked() }
        }
    }
}
