import SwiftUI

/// The popover content shown when clicking the menu bar icon.
struct MenuBarPopoverView: View {
    @Environment(AppViewModel.self) private var appVM
    @State private var tableFilter = ""
    @State private var expandedTable: String?

    var body: some View {
        ScrollView {
            VStack(spacing: 12) {
                connectionHeader
                tablesSection
                recentQueries
                quickActions
                statsBar
            }
            .padding(12)
        }
        .frame(width: 380, height: 520)
        .background(SbqlTheme.Colors.surface)
    }

    // MARK: - Connection Header

    private var connectionHeader: some View {
        Group {
            if let conn = appVM.connections.activeConnection {
                HStack(spacing: 8) {
                    Circle()
                        .fill(SbqlTheme.Colors.success)
                        .frame(width: 8, height: 8)
                    Text(conn.name)
                        .font(SbqlTheme.Typography.bodyMedium)
                        .foregroundStyle(SbqlTheme.Colors.textPrimary)
                    BadgePillView(text: conn.backend.abbreviation, color: conn.backend.color, fontWeight: .bold, fontDesign: .monospaced)
                    Spacer()
                    if let d = appVM.editor.lastQueryDuration {
                        let ms = d.components.seconds * 1000 + d.components.attoseconds / 1_000_000_000_000_000
                        BadgePillView(text: "\(ms)ms", color: SbqlTheme.Colors.success)
                    }
                }
                Text(conn.displaySubtitle)
                    .font(SbqlTheme.Typography.caption)
                    .foregroundStyle(SbqlTheme.Colors.textTertiary)
                    .frame(maxWidth: .infinity, alignment: .leading)
            } else {
                HStack {
                    Circle()
                        .fill(SbqlTheme.Colors.textTertiary)
                        .frame(width: 8, height: 8)
                    Text("Not connected")
                        .font(SbqlTheme.Typography.bodyMedium)
                        .foregroundStyle(SbqlTheme.Colors.textSecondary)
                    Spacer()
                }
            }
        }
        .padding(10)
        .background(SbqlTheme.Colors.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    // MARK: - Tables Section

    private var filteredTables: [TableEntryModel] {
        guard !tableFilter.isEmpty else { return appVM.connections.tables }
        return appVM.connections.tables.filter {
            $0.name.localizedCaseInsensitiveContains(tableFilter)
        }
    }

    private var tablesSection: some View {
        VStack(spacing: 6) {
            HStack {
                Text("Tables")
                    .font(SbqlTheme.Typography.captionBold)
                    .foregroundStyle(SbqlTheme.Colors.accent.opacity(0.7))
                Text("(\(appVM.connections.tables.count))")
                    .font(SbqlTheme.Typography.caption)
                    .foregroundStyle(SbqlTheme.Colors.textTertiary)
                Spacer()
            }

            if !appVM.connections.tables.isEmpty {
                SearchFieldView(text: $tableFilter, placeholder: "Filter tables…")

                VStack(spacing: 2) {
                    ForEach(filteredTables.prefix(20)) { table in
                        tableRow(table)
                    }
                    if filteredTables.count > 20 {
                        Text("… and \(filteredTables.count - 20) more")
                            .font(SbqlTheme.Typography.caption)
                            .foregroundStyle(SbqlTheme.Colors.textTertiary)
                            .padding(.top, 4)
                    }
                }
            } else {
                Text("Connect to see tables")
                    .font(SbqlTheme.Typography.caption)
                    .foregroundStyle(SbqlTheme.Colors.textTertiary)
                    .padding(.vertical, 8)
            }
        }
        .padding(10)
        .background(SbqlTheme.Colors.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    private func tableRow(_ table: TableEntryModel) -> some View {
        VStack(spacing: 0) {
            Button {
                withAnimation(.easeOut(duration: 0.15)) {
                    expandedTable = expandedTable == table.id ? nil : table.id
                }
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: expandedTable == table.id ? "chevron.down" : "chevron.right")
                        .font(.system(size: 8, weight: .bold))
                        .foregroundStyle(SbqlTheme.Colors.textTertiary)
                        .frame(width: 12)

                    Image(systemName: "tablecells")
                        .font(.system(size: 9))
                        .foregroundStyle(SbqlTheme.Colors.accent.opacity(0.5))

                    Text(table.name)
                        .font(SbqlTheme.Typography.caption)
                        .foregroundStyle(SbqlTheme.Colors.textPrimary)
                        .lineLimit(1)

                    Spacer()

                    // Show cached info if tab exists
                    if let tab = appVM.results.tabs.first(where: { $0.id == table.qualified }) {
                        Text("\(tab.result.rowCount) rows")
                            .font(.system(size: 9))
                            .foregroundStyle(SbqlTheme.Colors.textTertiary)
                    }
                }
                .padding(.vertical, 4)
                .padding(.horizontal, 6)
            }
            .buttonStyle(.hover)

            // Expanded detail
            if expandedTable == table.id {
                tableDetail(table)
            }
        }
    }

    private func tableDetail(_ table: TableEntryModel) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            // Show columns from diagram data if available
            if let diagramTable = appVM.diagram.diagramData.tables.first(where: { $0.name == table.name }) {
                ForEach(diagramTable.columns.prefix(8)) { col in
                    HStack(spacing: 4) {
                        if col.isPk {
                            Image(systemName: "key.fill")
                                .font(.system(size: 7))
                                .foregroundStyle(SbqlTheme.Colors.warning)
                        } else if col.isFk {
                            Image(systemName: "link")
                                .font(.system(size: 7))
                                .foregroundStyle(SbqlTheme.Colors.accent.opacity(0.5))
                        } else {
                            Color.clear.frame(width: 8)
                        }

                        Text(col.name)
                            .font(.system(size: 10, design: .monospaced))
                            .foregroundStyle(SbqlTheme.Colors.textPrimary)
                            .lineLimit(1)

                        Spacer()

                        Text(col.dataType)
                            .font(.system(size: 9, design: .monospaced))
                            .foregroundStyle(SbqlTheme.Colors.textTertiary)
                            .lineLimit(1)
                    }
                }

                if diagramTable.columns.count > 8 {
                    Text("… \(diagramTable.columns.count - 8) more columns")
                        .font(.system(size: 9))
                        .foregroundStyle(SbqlTheme.Colors.textTertiary)
                }

                // Actions
                HStack(spacing: 8) {
                    Button("SELECT *") {
                        let sql = "SELECT * FROM \(table.qualified)"
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString(sql, forType: .string)
                    }
                    .font(.system(size: 9, weight: .medium))
                    .foregroundStyle(SbqlTheme.Colors.accent)

                    Button("Open in App") {
                        NSApp.activate(ignoringOtherApps: true)
                        Task { await appVM.selectTable(table) }
                    }
                    .font(.system(size: 9, weight: .medium))
                    .foregroundStyle(SbqlTheme.Colors.accent)
                }
                .padding(.top, 4)
                .buttonStyle(.hover)
            } else {
                Text("Load diagram to see columns")
                    .font(.system(size: 9))
                    .foregroundStyle(SbqlTheme.Colors.textTertiary)
            }
        }
        .padding(.leading, 30)
        .padding(.trailing, 6)
        .padding(.bottom, 6)
    }

    // MARK: - Recent Queries

    private var recentQueries: some View {
        VStack(spacing: 6) {
            HStack {
                Text("Recent Queries")
                    .font(SbqlTheme.Typography.captionBold)
                    .foregroundStyle(SbqlTheme.Colors.accent.opacity(0.7))
                Spacer()
            }

            if appVM.queryHistory.entries.isEmpty {
                Text("No queries yet")
                    .font(SbqlTheme.Typography.caption)
                    .foregroundStyle(SbqlTheme.Colors.textTertiary)
                    .padding(.vertical, 4)
            } else {
                VStack(spacing: 2) {
                    ForEach(appVM.queryHistory.entries.prefix(5)) { entry in
                        Button {
                            NSPasteboard.general.clearContents()
                            NSPasteboard.general.setString(entry.sql, forType: .string)
                        } label: {
                            HStack(spacing: 6) {
                                Text(entry.sqlPreview)
                                    .font(.system(size: 10, design: .monospaced))
                                    .foregroundStyle(SbqlTheme.Colors.textPrimary)
                                    .lineLimit(1)
                                Spacer()
                                Text("\(entry.durationMs)ms")
                                    .font(.system(size: 9))
                                    .foregroundStyle(SbqlTheme.Colors.success)
                                Image(systemName: "doc.on.clipboard")
                                    .font(.system(size: 9))
                                    .foregroundStyle(SbqlTheme.Colors.textTertiary)
                            }
                            .padding(.vertical, 3)
                            .padding(.horizontal, 6)
                        }
                        .buttonStyle(.hover)
                    }
                }
            }
        }
        .padding(10)
        .background(SbqlTheme.Colors.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    // MARK: - Quick Actions

    private var quickActions: some View {
        HStack(spacing: 6) {
            actionButton("Open App", icon: "macwindow") {
                NSApp.activate(ignoringOtherApps: true)
            }
            actionButton("Cmd+K", icon: "command") {
                NSApp.activate(ignoringOtherApps: true)
                appVM.isCommandPaletteOpen = true
            }
            actionButton("Paste & Run", icon: "doc.on.clipboard") {
                if let sql = NSPasteboard.general.string(forType: .string) {
                    NSApp.activate(ignoringOtherApps: true)
                    appVM.editor.sqlText = sql
                    appVM.editor.isVisible = true
                    Task { await appVM.runQuery() }
                }
            }
        }
    }

    private func actionButton(_ label: String, icon: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            VStack(spacing: 4) {
                Image(systemName: icon)
                    .font(.system(size: 14))
                    .foregroundStyle(SbqlTheme.Colors.accent)
                Text(label)
                    .font(.system(size: 9, weight: .medium))
                    .foregroundStyle(SbqlTheme.Colors.textSecondary)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 8)
            .background(SbqlTheme.Colors.surfaceElevated)
            .clipShape(RoundedRectangle(cornerRadius: 8))
        }
        .buttonStyle(.hover)
    }

    // MARK: - Stats

    private var statsBar: some View {
        HStack(spacing: 0) {
            statItem("\(appVM.queryHistory.entries.count)", label: "queries")
            statItem("\(appVM.connections.tables.count)", label: "tables")
            statItem("\(appVM.results.tabs.count)", label: "tabs")
            statItem(appVM.connections.activeConnection != nil ? "●" : "○", label: "status")
        }
        .padding(.vertical, 6)
        .background(SbqlTheme.Colors.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    private func statItem(_ value: String, label: String) -> some View {
        VStack(spacing: 2) {
            Text(value)
                .font(.system(size: 14, weight: .semibold, design: .rounded))
                .foregroundStyle(SbqlTheme.Colors.textPrimary)
            Text(label)
                .font(.system(size: 9))
                .foregroundStyle(SbqlTheme.Colors.textTertiary)
        }
        .frame(maxWidth: .infinity)
    }
}
