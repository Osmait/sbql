import SwiftUI

struct CommandPalette: View {
    @Environment(AppViewModel.self) private var appVM
    @State private var searchText = ""
    @State private var selectedIndex = 0
    @State private var cachedItems: [CommandItem] = []
    @FocusState private var isSearchFocused: Bool

    var body: some View {
        VStack(spacing: 0) {
            // Search
            SearchFieldView(
                text: $searchText,
                placeholder: "Type a command, table, or query…",
                font: SbqlTheme.Typography.body,
                iconSize: 14,
                icon: "command",
                iconColor: SbqlTheme.Colors.accent
            )
            .focused($isSearchFocused)
            .onSubmit { executeSelected() }
            .padding(SbqlTheme.Spacing.lg)

            Divider().background(SbqlTheme.Colors.border)

            // Results
            if filteredItems.isEmpty {
                EmptyStateView(icon: "magnifyingglass", title: "No results")
            } else {
                ScrollViewReader { proxy in
                    ScrollView {
                        LazyVStack(spacing: 0) {
                            ForEach(Array(filteredItems.enumerated()), id: \.element.id) { index, item in
                                commandRow(item, isSelected: index == selectedIndex)
                                    .id(index)
                                    .onTapGesture { execute(item) }
                            }
                        }
                    }
                    .onChange(of: selectedIndex) { _, idx in
                        proxy.scrollTo(idx, anchor: .center)
                    }
                }
            }
        }
        .frame(width: 520, height: 400)
        .background(SbqlTheme.Colors.surface)
        .onAppear {
            isSearchFocused = true
            cachedItems = buildItems()
        }
        .onChange(of: searchText) { selectedIndex = 0 }
    }

    // MARK: - Commands

    private func buildItems() -> [CommandItem] {
        var items: [CommandItem] = []

        // Static commands
        items.append(CommandItem(title: "Toggle Sidebar", subtitle: nil, icon: "sidebar.leading", category: .command, shortcut: "⌃⌘S", action: { appVM.isSidebarVisible.toggle() }))
        items.append(CommandItem(title: "Toggle SQL Editor", subtitle: nil, icon: "chevron.up.chevron.down", category: .command, shortcut: "⌘E", action: { appVM.editor.isVisible.toggle() }))
        items.append(CommandItem(title: "New Connection", subtitle: nil, icon: "plus.circle", category: .command, shortcut: nil, action: {
            appVM.connections.editingConnection = Connection.newPostgres()
            appVM.connections.isShowingConnectionForm = true
        }))
        items.append(CommandItem(title: "Query History", subtitle: nil, icon: "clock.arrow.circlepath", category: .command, shortcut: nil, action: { appVM.isShowingHistory = true }))
        items.append(CommandItem(title: "Saved Queries", subtitle: nil, icon: "bookmark", category: .command, shortcut: nil, action: { appVM.isShowingSavedQueries = true }))
        items.append(CommandItem(title: "Switch to Diagram", subtitle: nil, icon: "rectangle.3.group", category: .command, shortcut: nil, action: {
            appVM.activeTab = .diagram
            Task { await appVM.loadDiagram() }
        }))
        items.append(CommandItem(title: "Switch to Query", subtitle: nil, icon: "text.and.command.macwindow", category: .command, shortcut: nil, action: { appVM.activeTab = .query }))
        items.append(CommandItem(title: "Refresh Tables", subtitle: nil, icon: "arrow.clockwise", category: .command, shortcut: nil, action: { Task { await appVM.refreshTables() } }))
        items.append(CommandItem(title: "Settings", subtitle: nil, icon: "gearshape", category: .command, shortcut: "⌘,", action: { NSApp.sendAction(Selector(("showSettingsWindow:")), to: nil, from: nil) }))

        // Tables
        for table in appVM.connections.tables {
            items.append(CommandItem(title: table.name, subtitle: table.schema, icon: "tablecells", category: .table, shortcut: nil, action: {
                Task { await appVM.selectTable(table) }
            }))
        }

        // Connections
        for conn in appVM.connections.connections {
            items.append(CommandItem(title: conn.name, subtitle: conn.displaySubtitle, icon: "server.rack", category: .connection, shortcut: nil, action: {
                Task { await appVM.connect(id: conn.id) }
            }))
        }

        // Saved queries
        for query in appVM.savedQueries.queries.prefix(15) {
            items.append(CommandItem(title: query.name, subtitle: query.sqlPreview, icon: "bookmark", category: .savedQuery, shortcut: nil, action: {
                appVM.editor.sqlText = query.sql
                appVM.editor.isVisible = true
            }))
        }

        // History
        for entry in appVM.queryHistory.entries.prefix(10) {
            items.append(CommandItem(title: entry.sqlPreview, subtitle: entry.connectionName, icon: "clock", category: .history, shortcut: nil, action: {
                appVM.editor.sqlText = entry.sql
                appVM.editor.isVisible = true
            }))
        }

        return items
    }

    private var filteredItems: [CommandItem] {
        guard !searchText.isEmpty else { return cachedItems }
        let q = searchText.lowercased()
        return cachedItems.filter {
            $0.title.lowercased().contains(q) ||
            ($0.subtitle?.lowercased().contains(q) ?? false)
        }
    }

    private func moveSelection(_ delta: Int) {
        let count = filteredItems.count
        guard count > 0 else { return }
        selectedIndex = (selectedIndex + delta + count) % count
    }

    private func executeSelected() {
        guard selectedIndex < filteredItems.count else { return }
        execute(filteredItems[selectedIndex])
    }

    private func execute(_ item: CommandItem) {
        appVM.isCommandPaletteOpen = false
        item.action()
    }

    // MARK: - Row

    private func commandRow(_ item: CommandItem, isSelected: Bool) -> some View {
        HStack(spacing: SbqlTheme.Spacing.sm) {
            Image(systemName: item.icon)
                .font(.system(size: 12))
                .foregroundStyle(SbqlTheme.Colors.accent)
                .frame(width: 20)

            VStack(alignment: .leading, spacing: 1) {
                Text(item.title)
                    .font(SbqlTheme.Typography.body)
                    .foregroundStyle(SbqlTheme.Colors.textPrimary)
                    .lineLimit(1)
                if let sub = item.subtitle {
                    Text(sub)
                        .font(SbqlTheme.Typography.caption)
                        .foregroundStyle(SbqlTheme.Colors.textTertiary)
                        .lineLimit(1)
                }
            }

            Spacer()

            Text(item.category.rawValue)
                .font(.system(size: 9))
                .foregroundStyle(SbqlTheme.Colors.textTertiary)

            if let shortcut = item.shortcut {
                BadgePillView(
                    text: shortcut,
                    color: SbqlTheme.Colors.accent.opacity(0.5),
                    fontDesign: .monospaced
                )
            }
        }
        .padding(.horizontal, SbqlTheme.Spacing.lg)
        .padding(.vertical, SbqlTheme.Spacing.sm)
        .background(isSelected ? SbqlTheme.Colors.accent.opacity(0.1) : Color.clear)
    }
}
