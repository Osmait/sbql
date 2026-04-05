import SwiftUI

struct ResultsView: View {
    @Environment(AppViewModel.self) private var appVM
    @Environment(ThemeManager.self) private var theme

    var body: some View {
        let result = appVM.results.currentResult

        Group {
            if result.isEmpty {
                emptyState
                    .transition(.opacity)
            } else {
                ResultsTableView(activeTheme: theme.activeThemeName)
                    .id("\(result.columns)\(theme.activeThemeName)")
                    .transition(.opacity)
            }
        }
        .animation(SbqlTheme.Animations.gentle, value: result.isEmpty)
    }

    private var emptyState: some View {
        VStack(spacing: SbqlTheme.Spacing.md) {
            EmptyStateView(
                icon: "text.and.command.macwindow",
                title: "Run a query to see results"
            )
            BadgePillView(
                text: "Cmd+Enter",
                color: SbqlTheme.Colors.accent.opacity(0.6),
                fontWeight: .semibold
            )
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(SbqlTheme.Colors.surface)
    }
}
