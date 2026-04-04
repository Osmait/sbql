import SwiftUI

struct QueryHistorySection: View {
    @Environment(AppViewModel.self) private var appVM

    private static let relativeFormatter: RelativeDateTimeFormatter = {
        let f = RelativeDateTimeFormatter()
        f.unitsStyle = .abbreviated
        return f
    }()

    var body: some View {
        sectionHeader("HISTORY") {
            appVM.queryHistory.clearHistory()
        }

        // Search field (show when 5+ entries)
        if appVM.queryHistory.entries.count >= 5 {
            searchField
        }

        LazyVStack(spacing: 2) {
            ForEach(appVM.queryHistory.filteredEntries) { entry in
                historyRow(entry)
            }
        }
        .padding(.horizontal, SbqlTheme.Spacing.sm)
        .animation(SbqlTheme.Animations.gentle, value: appVM.queryHistory.filteredEntries.count)
    }

    // MARK: - Row

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
                    // Connection badge
                    Text(entry.connectionName)
                        .font(.system(size: 9, weight: .medium))
                        .foregroundStyle(SbqlTheme.Colors.accent)
                        .padding(.horizontal, SbqlTheme.Spacing.xs)
                        .padding(.vertical, 1)
                        .background(SbqlTheme.Colors.accent.opacity(0.12))
                        .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))

                    // Duration badge
                    Text("\(entry.durationMs)ms")
                        .font(.system(size: 9, weight: .medium))
                        .foregroundStyle(SbqlTheme.Colors.textTertiary)

                    Spacer()

                    // Relative time
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

    // MARK: - Search

    private var searchField: some View {
        HStack(spacing: SbqlTheme.Spacing.xs) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 10))
                .foregroundStyle(SbqlTheme.Colors.textTertiary)
            TextField("Filter history…", text: Binding(
                get: { appVM.queryHistory.searchText },
                set: { appVM.queryHistory.searchText = $0 }
            ))
            .textFieldStyle(.plain)
            .font(SbqlTheme.Typography.caption)
            .foregroundStyle(SbqlTheme.Colors.textPrimary)

            if !appVM.queryHistory.searchText.isEmpty {
                Button {
                    appVM.queryHistory.searchText = ""
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 10))
                        .foregroundStyle(SbqlTheme.Colors.textTertiary)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, SbqlTheme.Spacing.sm)
        .padding(.vertical, SbqlTheme.Spacing.xs)
        .background(SbqlTheme.Colors.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))
        .padding(.horizontal, SbqlTheme.Spacing.sm)
    }

    // MARK: - Section Header

    private func sectionHeader(_ title: String, clearAction: @escaping () -> Void) -> some View {
        HStack {
            Text(title)
                .font(SbqlTheme.Typography.captionBold)
                .foregroundStyle(SbqlTheme.Colors.accent.opacity(0.7))

            Spacer()

            Button(action: clearAction) {
                Image(systemName: "trash")
                    .font(.system(size: 10, weight: .bold))
                    .foregroundStyle(SbqlTheme.Colors.textTertiary)
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, SbqlTheme.Spacing.lg)
        .padding(.vertical, SbqlTheme.Spacing.xs)
    }
}
