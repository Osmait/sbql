import SwiftUI

struct QueryHistorySection: View {
    @Environment(AppViewModel.self) private var appVM

    private static let relativeFormatter: RelativeDateTimeFormatter = {
        let f = RelativeDateTimeFormatter()
        f.unitsStyle = .abbreviated
        return f
    }()

    var body: some View {
        SectionHeaderView(title: "HISTORY", action: {
            appVM.queryHistory.clearHistory()
        }, actionIcon: "trash")

        if appVM.queryHistory.entries.count >= 5 {
            SearchFieldView(
                text: Binding(
                    get: { appVM.queryHistory.searchText },
                    set: { appVM.queryHistory.searchText = $0 }
                ),
                placeholder: "Filter history…"
            )
            .padding(.horizontal, SbqlTheme.Spacing.sm)
        }

        LazyVStack(spacing: 2) {
            ForEach(appVM.queryHistory.filteredEntries) { entry in
                historyRow(entry)
            }
        }
        .padding(.horizontal, SbqlTheme.Spacing.sm)
        .animation(SbqlTheme.Animations.gentle, value: appVM.queryHistory.filteredEntries.count)
    }

    private func historyRow(_ entry: QueryHistoryEntry) -> some View {
        Button {
            appVM.editor.sqlText = entry.sql
            appVM.editor.isVisible = true
        } label: {
            VStack(alignment: .leading, spacing: SbqlTheme.Spacing.xxs) {
                Text(entry.sqlPreview)
                    .font(SbqlTheme.Typography.codeSmall)
                    .foregroundStyle(SbqlTheme.Colors.textPrimary)
                    .lineLimit(1)

                HStack(spacing: SbqlTheme.Spacing.xs) {
                    BadgePillView(text: entry.connectionName, color: SbqlTheme.Colors.accent)

                    Text("\(entry.durationMs)ms")
                        .font(.system(size: 9))
                        .foregroundStyle(SbqlTheme.Colors.textTertiary)

                    Spacer()

                    Text(Self.relativeFormatter.localizedString(for: entry.timestamp, relativeTo: Date()))
                        .font(.system(size: 9))
                        .foregroundStyle(SbqlTheme.Colors.textTertiary)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, SbqlTheme.Spacing.sm)
            .padding(.vertical, SbqlTheme.Spacing.xs)
            .background(SbqlTheme.Colors.surface)
            .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))
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
