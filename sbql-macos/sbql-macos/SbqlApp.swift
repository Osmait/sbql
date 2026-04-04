import SwiftUI

@main
struct SbqlApp: App {
    @State private var appVM = AppViewModel()

    var body: some Scene {
        WindowGroup {
            MainWindow()
                .environment(appVM)
                .environment(ThemeManager.shared)
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 1200, height: 800)

        Settings {
            SettingsView()
        }
    }
}
