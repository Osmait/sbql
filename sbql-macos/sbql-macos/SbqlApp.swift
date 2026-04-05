import SwiftUI

@main
struct SbqlApp: App {
    @State private var appVM = AppViewModel()
    @State private var menuBarManager: MenuBarManager?

    var body: some Scene {
        WindowGroup {
            MainWindow()
                .environment(appVM)
                .environment(ThemeManager.shared)
                .onAppear {
                    if menuBarManager == nil {
                        menuBarManager = MenuBarManager(appVM: appVM)
                    }
                }
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 1200, height: 800)

        Settings {
            SettingsView()
        }
    }
}
