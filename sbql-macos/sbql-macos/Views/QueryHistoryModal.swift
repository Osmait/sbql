import SwiftUI

/// Modal showing query history with search, click to load.
struct QueryHistoryModal: View {
    @Environment(AppViewModel.self) private var appVM

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Image(systemName: "clock.arrow.circlepath")
                    .font(.system(size: 14))
                    .foregroundStyle(SbqlTheme.Colors.accent)
                Text("Query History")
                    .font(SbqlTheme.Typography.title)
                    .foregroundStyle(SbqlTheme.Colors.textPrimary)
                Spacer()
                if !appVM.queryHistory.entries.isEmpty {
                    Button("Clear All") {
                        appVM.queryHistory.clearHistory()
                    }
                    .buttonStyle(.hover)
                    .foregroundStyle(SbqlTheme.Colors.danger)
                    .font(SbqlTheme.Typography.caption)
                }
                Button { appVM.isShowingHistory = false } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 16))
                        .foregroundStyle(SbqlTheme.Colors.textTertiary)
                }
                .buttonStyle(.hoverIcon)
            }
            .padding(SbqlTheme.Spacing.lg)

            // Search
            SearchFieldView(
                text: Binding(
                    get: { appVM.queryHistory.searchText },
                    set: { appVM.queryHistory.searchText = $0 }
                ),
                placeholder: "Search history…",
                font: SbqlTheme.Typography.body,
                iconSize: 11
            )
            .padding(.horizontal, SbqlTheme.Spacing.lg)
            .padding(.bottom, SbqlTheme.Spacing.sm)

            Divider().background(SbqlTheme.Colors.border)

            // List
            if appVM.queryHistory.filteredEntries.isEmpty {
                EmptyStateView(icon: "clock", title: "No history yet")
            } else {
                ScrollView {
                    LazyVStack(spacing: 2) {
                        ForEach(appVM.queryHistory.filteredEntries) { entry in
                            historyRow(entry)
                        }
                    }
                    .padding(SbqlTheme.Spacing.sm)
                }
            }
        }
        .frame(width: 600, height: 500)
        .background(SbqlTheme.Colors.surface)
    }

    private func historyRow(_ entry: QueryHistoryEntry) -> some View {
        Button {
            appVM.editor.sqlText = entry.sql
            appVM.editor.isVisible = true
            appVM.isShowingHistory = false
        } label: {
            VStack(alignment: .leading, spacing: 3) {
                Text(entry.sqlPreview)
                    .font(SbqlTheme.Typography.codeSmall)
                    .foregroundStyle(SbqlTheme.Colors.textPrimary)
                    .lineLimit(2)

                HStack(spacing: SbqlTheme.Spacing.sm) {
                    // Connection badge
                    BadgePillView(text: entry.connectionName, color: SbqlTheme.Colors.accent)

                    // Duration
                    Text("\(entry.durationMs)ms")
                        .font(.system(size: 9))
                        .foregroundStyle(SbqlTheme.Colors.success)

                    // Row count
                    Text("\(entry.rowCount) rows")
                        .font(.system(size: 9))
                        .foregroundStyle(SbqlTheme.Colors.textTertiary)

                    Spacer()

                    // Relative time
                    Text(entry.timestamp, style: .relative)
                        .font(.system(size: 9))
                        .foregroundStyle(SbqlTheme.Colors.textTertiary)
                }
            }
            .padding(SbqlTheme.Spacing.sm)
            .background(SbqlTheme.Colors.surfaceElevated.opacity(0.5))
            .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium))
        }
        .buttonStyle(.hover)
        .contextMenu {
            Button("Copy SQL") {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(entry.sql, forType: .string)
            }
            Button("Save as Query") {
                appVM.savedQueries.saveSheetSQL = entry.sql
                appVM.savedQueries.isShowingSaveSheet = true
            }
        }
    }
}
