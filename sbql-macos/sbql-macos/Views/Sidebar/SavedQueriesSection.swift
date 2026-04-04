import SwiftUI

struct SavedQueriesSection: View {
    @Environment(AppViewModel.self) private var appVM

    @State private var renamingId: UUID?
    @State private var renameText: String = ""

    var body: some View {
        sectionHeader("SAVED QUERIES") {
            appVM.savedQueries.saveSheetSQL = appVM.editor.sqlText
            appVM.savedQueries.isShowingSaveSheet = true
        }

        // Search field (show when 3+ queries)
        if appVM.savedQueries.queries.count >= 3 {
            searchField
        }

        LazyVStack(spacing: 2) {
            ForEach(appVM.savedQueries.filteredQueries) { query in
                queryRow(query)
            }
        }
        .padding(.horizontal, SbqlTheme.Spacing.sm)
        .animation(SbqlTheme.Animations.gentle, value: appVM.savedQueries.filteredQueries.count)
    }

    // MARK: - Row

    private func queryRow(_ query: SavedQuery) -> some View {
        Button {
            appVM.editor.sqlText = query.sql
            appVM.editor.isVisible = true
        } label: {
            VStack(alignment: .leading, spacing: SbqlTheme.Spacing.xxs) {
                if renamingId == query.id {
                    TextField("Name", text: $renameText, onCommit: {
                        if !renameText.isEmpty {
                            appVM.savedQueries.rename(id: query.id, newName: renameText)
                        }
                        renamingId = nil
                    })
                    .textFieldStyle(.plain)
                    .font(SbqlTheme.Typography.bodyMedium)
                    .foregroundStyle(SbqlTheme.Colors.textPrimary)
                } else {
                    Text(query.name)
                        .font(SbqlTheme.Typography.bodyMedium)
                        .foregroundStyle(SbqlTheme.Colors.textPrimary)
                        .lineLimit(1)
                }

                Text(query.sqlPreview)
                    .font(SbqlTheme.Typography.caption)
                    .foregroundStyle(SbqlTheme.Colors.textTertiary)
                    .lineLimit(1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, SbqlTheme.Spacing.sm)
            .padding(.vertical, SbqlTheme.Spacing.xs)
            .background(SbqlTheme.Colors.surface)
            .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))
        }
        .buttonStyle(.plain)
        .contextMenu {
            Button("Rename") {
                renameText = query.name
                renamingId = query.id
            }
            Button("Duplicate") {
                appVM.savedQueries.duplicate(id: query.id)
            }
            Button("Copy SQL") {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(query.sql, forType: .string)
            }
            Divider()
            Button("Delete", role: .destructive) {
                appVM.savedQueries.delete(id: query.id)
            }
        }
    }

    // MARK: - Search

    private var searchField: some View {
        HStack(spacing: SbqlTheme.Spacing.xs) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 10))
                .foregroundStyle(SbqlTheme.Colors.textTertiary)
            TextField("Filter saved queries…", text: Binding(
                get: { appVM.savedQueries.searchText },
                set: { appVM.savedQueries.searchText = $0 }
            ))
            .textFieldStyle(.plain)
            .font(SbqlTheme.Typography.caption)
            .foregroundStyle(SbqlTheme.Colors.textPrimary)

            if !appVM.savedQueries.searchText.isEmpty {
                Button {
                    appVM.savedQueries.searchText = ""
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

    private func sectionHeader(_ title: String, action: @escaping () -> Void) -> some View {
        HStack {
            Text(title)
                .font(SbqlTheme.Typography.captionBold)
                .foregroundStyle(SbqlTheme.Colors.accent.opacity(0.7))

            Spacer()

            Button(action: action) {
                Image(systemName: "plus")
                    .font(.system(size: 10, weight: .bold))
                    .foregroundStyle(SbqlTheme.Colors.accent)
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, SbqlTheme.Spacing.lg)
        .padding(.vertical, SbqlTheme.Spacing.xs)
    }
}
