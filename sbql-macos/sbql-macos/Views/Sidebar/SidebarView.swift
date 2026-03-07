import SwiftUI

struct SidebarView: View {
    @Environment(AppViewModel.self) private var appVM

    var body: some View {
        VStack(spacing: 0) {
            // Padding for traffic lights
            Color.clear.frame(height: 38)

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

                    sectionHeader("TABLES", action: nil)

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
                            TableRow(table: table)
                        }
                    }
                    .padding(.horizontal, SbqlTheme.Spacing.sm)
                }
            }
        }
        .background(.ultraThinMaterial)
        .background(SbqlTheme.Colors.surface.opacity(0.5))
        .overlay(alignment: .trailing) {
            SbqlTheme.Colors.border.frame(width: 1)
        }
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
                .foregroundStyle(SbqlTheme.Colors.textTertiary)

            Spacer()

            if let action {
                Button(action: action) {
                    Image(systemName: "plus")
                        .font(.system(size: 10, weight: .bold))
                        .foregroundStyle(SbqlTheme.Colors.textSecondary)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, SbqlTheme.Spacing.lg)
        .padding(.vertical, SbqlTheme.Spacing.xs)
    }
}
