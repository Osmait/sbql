import SwiftUI

struct ResultsView: View {
    @Environment(AppViewModel.self) private var appVM

    var body: some View {
        let result = appVM.results.currentResult

        if result.isEmpty {
            emptyState
        } else {
            ResultsTableView()
                .id(result.columns) // force rebuild when columns change
        }
    }

    private var emptyState: some View {
        VStack(spacing: SbqlTheme.Spacing.md) {
            Image(systemName: "text.and.command.macwindow")
                .font(.system(size: 32))
                .foregroundStyle(SbqlTheme.Colors.textTertiary)

            Text("Run a query to see results")
                .font(SbqlTheme.Typography.body)
                .foregroundStyle(SbqlTheme.Colors.textSecondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(SbqlTheme.Colors.background)
    }
}
