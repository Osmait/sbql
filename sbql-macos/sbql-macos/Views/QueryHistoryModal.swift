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
                    .buttonStyle(.plain)
                    .foregroundStyle(SbqlTheme.Colors.danger)
                    .font(SbqlTheme.Typography.caption)
                }
                Button { appVM.isShowingHistory = false } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 16))
                        .foregroundStyle(SbqlTheme.Colors.textTertiary)
                }
                .buttonStyle(.plain)
            }
            .padding(SbqlTheme.Spacing.lg)

            // Search
            HStack(spacing: SbqlTheme.Spacing.xs) {
                Image(systemName: "magnifyingglass")
                    .font(.system(size: 11))
                    .foregroundStyle(SbqlTheme.Colors.textTertiary)
                TextField("Search history…", text: Binding(
                    get: { appVM.queryHistory.searchText },
                    set: { appVM.queryHistory.searchText = $0 }
                ))
                .textFieldStyle(.plain)
                .font(SbqlTheme.Typography.body)
            }
            .padding(.horizontal, SbqlTheme.Spacing.md)
            .padding(.vertical, SbqlTheme.Spacing.sm)
            .background(SbqlTheme.Colors.surfaceElevated)
            .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium))
            .padding(.horizontal, SbqlTheme.Spacing.lg)
            .padding(.bottom, SbqlTheme.Spacing.sm)

            Divider().background(SbqlTheme.Colors.border)

            // List
            if appVM.queryHistory.filteredEntries.isEmpty {
                VStack(spacing: SbqlTheme.Spacing.sm) {
                    Image(systemName: "clock")
                        .font(.system(size: 28))
                        .foregroundStyle(SbqlTheme.Colors.textTertiary)
                    Text("No history yet")
                        .font(SbqlTheme.Typography.body)
                        .foregroundStyle(SbqlTheme.Colors.textSecondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
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
                    Text(entry.connectionName)
                        .font(.system(size: 9, weight: .medium))
                        .foregroundStyle(SbqlTheme.Colors.accent)
                        .padding(.horizontal, 4)
                        .padding(.vertical, 1)
                        .background(SbqlTheme.Colors.accent.opacity(0.1))
                        .clipShape(RoundedRectangle(cornerRadius: 3))

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
        .buttonStyle(.plain)
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
