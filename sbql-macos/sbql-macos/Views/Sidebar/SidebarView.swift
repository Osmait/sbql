import SwiftUI

struct SidebarView: View {
    @Environment(AppViewModel.self) private var appVM

    private var allSameSchema: Bool {
        let schemas = Set(appVM.connections.tables.map { $0.schema })
        return schemas.count <= 1
    }

    private var commonSchema: String {
        appVM.connections.tables.first?.schema ?? "public"
    }

    var body: some View {
        VStack(spacing: 0) {
            // Small top padding (header is above)
            Color.clear.frame(height: SbqlTheme.Spacing.sm)

            // Connections section header + search
            SectionHeaderView(title: "CONNECTIONS") {
                appVM.connections.editingConnection = Connection.newPostgres()
                appVM.connections.isShowingConnectionForm = true
            }

            // Connection search (show when 4+ connections)
            if appVM.connections.connections.count >= 4 {
                SearchFieldView(
                    text: Binding(
                        get: { appVM.connections.connectionFilter },
                        set: { appVM.connections.connectionFilter = $0 }
                    ),
                    placeholder: "Search connections…"
                )
                .padding(.horizontal, SbqlTheme.Spacing.sm)
            }

            ScrollView {
                // Group connections by backend
                LazyVStack(spacing: 2) {
                    ForEach(appVM.connections.groupedConnections, id: \.backend) { group in
                        backendGroupHeader(group.backend, count: group.connections.count)
                        ForEach(group.connections) { conn in
                            ConnectionRow(connection: conn)
                        }
                    }
                }
                .padding(.horizontal, SbqlTheme.Spacing.sm)

                if !appVM.connections.tables.isEmpty {
                    Divider()
                        .transition(.opacity)
                        .background(SbqlTheme.Colors.border)
                        .padding(.vertical, SbqlTheme.Spacing.sm)

                    if allSameSchema {
                        SectionHeaderView(
                            title: "\(commonSchema.uppercased()) (\(appVM.connections.tables.count) tables)"
                        )
                    } else {
                        SectionHeaderView(title: "TABLES")
                    }

                    // Table filter
                    SearchFieldView(
                        text: Binding(
                            get: { appVM.connections.tableFilter },
                            set: { appVM.connections.tableFilter = $0 }
                        ),
                        placeholder: "Filter tables…"
                    )
                    .padding(.horizontal, SbqlTheme.Spacing.sm)

                    LazyVStack(spacing: 2) {
                        ForEach(appVM.connections.filteredTables) { table in
                            TableRow(table: table, showSchema: !allSameSchema)
                                .transition(.opacity.combined(with: .move(edge: .top)))
                        }
                    }
                    .padding(.horizontal, SbqlTheme.Spacing.sm)
                    .animation(SbqlTheme.Animations.gentle, value: appVM.connections.filteredTables.count)
                }

                // History and Saved Queries moved to header buttons
            }
            .animation(SbqlTheme.Animations.smooth, value: appVM.connections.tables.count)
        }
        // Background and clipping handled by the island container in MainWindow
        .sheet(isPresented: Binding(
            get: { appVM.connections.isShowingConnectionForm },
            set: { appVM.connections.isShowingConnectionForm = $0 }
        )) {
            if let conn = appVM.connections.editingConnection {
                ConnectionFormSheet(connection: conn)
            }
        }
        // Save query sheet moved to MainWindow
    }

    private func backendGroupHeader(_ backend: Connection.Backend, count: Int) -> some View {
        HStack(spacing: SbqlTheme.Spacing.xs) {
            Circle().fill(backend.color).frame(width: 6, height: 6)
            Text("\(backend.displayLabel) (\(count))")
                .font(.system(size: 10, weight: .semibold))
                .foregroundStyle(backend.color)
            Spacer()
        }
        .padding(.horizontal, SbqlTheme.Spacing.lg)
        .padding(.top, SbqlTheme.Spacing.sm)
        .padding(.bottom, SbqlTheme.Spacing.xxs)
    }

}
