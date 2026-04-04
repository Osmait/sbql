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
            Image(systemName: "text.and.command.macwindow")
                .font(.system(size: 32))
                .foregroundStyle(SbqlTheme.Colors.accent.opacity(0.4))

            Text("Run a query to see results")
                .font(SbqlTheme.Typography.body)
                .foregroundStyle(SbqlTheme.Colors.textSecondary)

            Text("Cmd+Enter")
                .font(SbqlTheme.Typography.captionBold)
                .foregroundStyle(SbqlTheme.Colors.accent.opacity(0.6))
                .padding(.horizontal, SbqlTheme.Spacing.sm)
                .padding(.vertical, SbqlTheme.Spacing.xs)
                .background(SbqlTheme.Colors.accent.opacity(0.08))
                .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(SbqlTheme.Colors.surface)
    }
}
