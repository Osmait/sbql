import SwiftUI

/// Modal showing saved queries with search, click to load, CRUD.
struct SavedQueriesModal: View {
    @Environment(AppViewModel.self) private var appVM
    @State private var editingId: UUID?
    @State private var editingName: String = ""

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Image(systemName: "bookmark")
                    .font(.system(size: 14))
                    .foregroundStyle(SbqlTheme.Colors.accent)
                Text("Saved Queries")
                    .font(SbqlTheme.Typography.title)
                    .foregroundStyle(SbqlTheme.Colors.textPrimary)
                Spacer()
                Button {
                    appVM.savedQueries.saveSheetSQL = appVM.editor.sqlText
                    appVM.savedQueries.isShowingSaveSheet = true
                } label: {
                    HStack(spacing: 3) {
                        Image(systemName: "plus")
                            .font(.system(size: 10, weight: .bold))
                        Text("Save Current")
                            .font(SbqlTheme.Typography.captionBold)
                    }
                    .foregroundStyle(SbqlTheme.Colors.accent)
                    .padding(.horizontal, SbqlTheme.Spacing.sm)
                    .padding(.vertical, SbqlTheme.Spacing.xs)
                    .background(SbqlTheme.Colors.accent.opacity(0.1))
                    .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))
                }
                .buttonStyle(.plain)
                .disabled(appVM.editor.sqlText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)

                Button { appVM.isShowingSavedQueries = false } label: {
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
                TextField("Search saved queries…", text: Binding(
                    get: { appVM.savedQueries.searchText },
                    set: { appVM.savedQueries.searchText = $0 }
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
            if appVM.savedQueries.filteredQueries.isEmpty {
                VStack(spacing: SbqlTheme.Spacing.sm) {
                    Image(systemName: "bookmark")
                        .font(.system(size: 28))
                        .foregroundStyle(SbqlTheme.Colors.textTertiary)
                    Text("No saved queries")
                        .font(SbqlTheme.Typography.body)
                        .foregroundStyle(SbqlTheme.Colors.textSecondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollView {
                    LazyVStack(spacing: 2) {
                        ForEach(appVM.savedQueries.filteredQueries) { query in
                            savedQueryRow(query)
                        }
                    }
                    .padding(SbqlTheme.Spacing.sm)
                }
            }
        }
        .frame(width: 600, height: 500)
        .background(SbqlTheme.Colors.surface)
    }

    private func savedQueryRow(_ query: SavedQuery) -> some View {
        Button {
            appVM.editor.sqlText = query.sql
            appVM.editor.isVisible = true
            appVM.isShowingSavedQueries = false
        } label: {
            VStack(alignment: .leading, spacing: 3) {
                if editingId == query.id {
                    TextField("Query name", text: $editingName, onCommit: {
                        appVM.savedQueries.rename(id: query.id, newName: editingName)
                        editingId = nil
                    })
                    .textFieldStyle(.plain)
                    .font(SbqlTheme.Typography.bodyMedium)
                    .foregroundStyle(SbqlTheme.Colors.textPrimary)
                } else {
                    Text(query.name)
                        .font(SbqlTheme.Typography.bodyMedium)
                        .foregroundStyle(SbqlTheme.Colors.textPrimary)
                }

                Text(query.sqlPreview)
                    .font(SbqlTheme.Typography.codeSmall)
                    .foregroundStyle(SbqlTheme.Colors.textTertiary)
                    .lineLimit(1)

                Text(query.updatedAt, style: .relative)
                    .font(.system(size: 9))
                    .foregroundStyle(SbqlTheme.Colors.textTertiary)
            }
            .padding(SbqlTheme.Spacing.sm)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(SbqlTheme.Colors.surfaceElevated.opacity(0.5))
            .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium))
        }
        .buttonStyle(.plain)
        .contextMenu {
            Button("Rename") {
                editingId = query.id
                editingName = query.name
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
}
