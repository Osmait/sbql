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

            // Connections section
            sectionHeader("CONNECTIONS") {
                appVM.connections.editingConnection = Connection.newPostgres()
                appVM.connections.isShowingConnectionForm = true
            }

            ScrollView {
                LazyVStack(spacing: 2) {
                    ForEach(appVM.connections.connections) { conn in
                        ConnectionRow(connection: conn)
                    }
                }
                .padding(.horizontal, SbqlTheme.Spacing.sm)

                if !appVM.connections.tables.isEmpty {
                    Divider()
                        .background(SbqlTheme.Colors.border)
                        .padding(.vertical, SbqlTheme.Spacing.sm)

                    if allSameSchema {
                        sectionHeader(
                            "\(commonSchema.uppercased()) (\(appVM.connections.tables.count) tables)",
                            action: nil
                        )
                    } else {
                        sectionHeader("TABLES", action: nil)
                    }

                    // Table filter
                    HStack(spacing: SbqlTheme.Spacing.xs) {
                        Image(systemName: "magnifyingglass")
                            .font(.system(size: 10))
                            .foregroundStyle(SbqlTheme.Colors.textTertiary)
                        TextField("Filter tables…", text: Binding(
                            get: { appVM.connections.tableFilter },
                            set: { appVM.connections.tableFilter = $0 }
                        ))
                        .textFieldStyle(.plain)
                        .font(SbqlTheme.Typography.caption)
                        .foregroundStyle(SbqlTheme.Colors.textPrimary)
                    }
                    .padding(.horizontal, SbqlTheme.Spacing.sm)
                    .padding(.vertical, SbqlTheme.Spacing.xs)
                    .background(SbqlTheme.Colors.surfaceElevated)
                    .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))
                    .padding(.horizontal, SbqlTheme.Spacing.sm)

                    LazyVStack(spacing: 2) {
                        ForEach(appVM.connections.filteredTables) { table in
                            TableRow(table: table, showSchema: !allSameSchema)
                        }
                    }
                    .padding(.horizontal, SbqlTheme.Spacing.sm)
                }
            }
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
    }

    private func sectionHeader(_ title: String, action: (() -> Void)?) -> some View {
        HStack {
            Text(title)
                .font(SbqlTheme.Typography.captionBold)
                .foregroundStyle(SbqlTheme.Colors.accent.opacity(0.7))

            Spacer()

            if let action {
                Button(action: action) {
                    Image(systemName: "plus")
                        .font(.system(size: 10, weight: .bold))
                        .foregroundStyle(SbqlTheme.Colors.accent)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, SbqlTheme.Spacing.lg)
        .padding(.vertical, SbqlTheme.Spacing.xs)
    }
}
