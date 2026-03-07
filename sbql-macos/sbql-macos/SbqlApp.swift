import SwiftUI

@main
struct SbqlApp: App {
    @State private var appVM = AppViewModel()

    var body: some Scene {
        WindowGroup {
            MainWindow()
                .environment(appVM)
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 1200, height: 800)
    }
}
