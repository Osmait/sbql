import SwiftUI

struct TablePreviewModal: View {
    @Environment(AppViewModel.self) private var appVM
    @State private var searchText = ""
    @State private var selectedIndex = 0
    @FocusState private var isSearchFocused: Bool

    private var filteredTables: [TableEntryModel] {
        guard !searchText.isEmpty else { return appVM.connections.tables }
        let q = searchText.lowercased()
        return appVM.connections.tables.filter { $0.name.lowercased().contains(q) }
    }

    private var selectedTable: TableEntryModel? {
        guard selectedIndex < filteredTables.count else { return nil }
        return filteredTables[selectedIndex]
    }

    private var cachedPreview: QueryResultData? {
        guard let table = selectedTable else { return nil }
        let tabId = "\(table.schema).\(table.name)"
        return appVM.results.tabs.first(where: { $0.id == tabId })?.result
    }

    var body: some View {
        VStack(spacing: 0) {
            // Search
            HStack(spacing: SbqlTheme.Spacing.sm) {
                Image(systemName: "magnifyingglass")
                    .font(.system(size: 14))
                    .foregroundStyle(SbqlTheme.Colors.accent)
                TextField("Search tables…", text: $searchText)
                    .textFieldStyle(.plain)
                    .font(SbqlTheme.Typography.body)
                    .focused($isSearchFocused)
                    .onSubmit { openSelectedTable() }
            }
            .padding(SbqlTheme.Spacing.lg)

            Divider().background(SbqlTheme.Colors.border)

            HStack(spacing: 0) {
                // Table list
                ScrollView {
                    LazyVStack(spacing: 0) {
                        ForEach(Array(filteredTables.enumerated()), id: \.element.id) { index, table in
                            HStack(spacing: SbqlTheme.Spacing.xs) {
                                Image(systemName: "tablecells")
                                    .font(.system(size: 10))
                                    .foregroundStyle(SbqlTheme.Colors.accent.opacity(0.5))
                                Text(table.name)
                                    .font(SbqlTheme.Typography.body)
                                    .foregroundStyle(SbqlTheme.Colors.textPrimary)
                                    .lineLimit(1)
                                Spacer()
                            }
                            .padding(.horizontal, SbqlTheme.Spacing.sm)
                            .padding(.vertical, SbqlTheme.Spacing.xs)
                            .background(index == selectedIndex ? SbqlTheme.Colors.accent.opacity(0.1) : Color.clear)
                            .onTapGesture { selectedIndex = index }
                            .onTapGesture(count: 2) { openTable(table) }
                        }
                    }
                }
                .frame(width: 200)

                SbqlTheme.Colors.border.frame(width: 1)

                // Preview
                VStack {
                    if let preview = cachedPreview, !preview.isEmpty {
                        previewContent(preview)
                    } else if let table = selectedTable {
                        VStack(spacing: SbqlTheme.Spacing.sm) {
                            Image(systemName: "arrow.turn.down.left")
                                .font(.system(size: 24))
                                .foregroundStyle(SbqlTheme.Colors.accent.opacity(0.3))
                            Text("Press Enter to open **\(table.name)**")
                                .font(SbqlTheme.Typography.body)
                                .foregroundStyle(SbqlTheme.Colors.textSecondary)
                        }
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                    } else {
                        Text("Select a table")
                            .font(SbqlTheme.Typography.body)
                            .foregroundStyle(SbqlTheme.Colors.textTertiary)
                            .frame(maxWidth: .infinity, maxHeight: .infinity)
                    }
                }
            }
        }
        .frame(width: 650, height: 450)
        .background(SbqlTheme.Colors.surface)
        .onAppear { isSearchFocused = true }
        .onChange(of: searchText) { selectedIndex = 0 }
    }

    private func previewContent(_ data: QueryResultData) -> some View {
        VStack(spacing: 0) {
            // Headers
            HStack(spacing: 0) {
                ForEach(data.columns.prefix(6), id: \.self) { col in
                    Text(col)
                        .font(SbqlTheme.Typography.captionBold)
                        .foregroundStyle(SbqlTheme.Colors.textSecondary)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.horizontal, 4)
                        .padding(.vertical, 3)
                }
            }
            .background(SbqlTheme.Colors.surfaceElevated)

            // Rows
            ForEach(Array(data.rows.prefix(10).enumerated()), id: \.offset) { _, row in
                HStack(spacing: 0) {
                    ForEach(Array(row.prefix(6).enumerated()), id: \.offset) { _, val in
                        Text(val)
                            .font(SbqlTheme.Typography.codeSmall)
                            .foregroundStyle(SbqlTheme.Colors.textPrimary)
                            .lineLimit(1)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.horizontal, 4)
                            .padding(.vertical, 2)
                    }
                }
            }

            Spacer()

            if data.rowCount > 10 {
                Text("… and \(data.rowCount - 10) more rows")
                    .font(SbqlTheme.Typography.caption)
                    .foregroundStyle(SbqlTheme.Colors.textTertiary)
                    .padding(SbqlTheme.Spacing.sm)
            }
        }
    }

    private func openSelectedTable() {
        guard let table = selectedTable else { return }
        openTable(table)
    }

    private func openTable(_ table: TableEntryModel) {
        appVM.isTablePreviewOpen = false
        Task { await appVM.selectTable(table) }
    }
}
