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
            sectionHeader("CONNECTIONS") {
                appVM.connections.editingConnection = Connection.newPostgres()
                appVM.connections.isShowingConnectionForm = true
            }

            // Connection search (show when 4+ connections)
            if appVM.connections.connections.count >= 4 {
                HStack(spacing: SbqlTheme.Spacing.xs) {
                    Image(systemName: "magnifyingglass")
                        .font(.system(size: 10))
                        .foregroundStyle(SbqlTheme.Colors.textTertiary)
                    TextField("Search connections…", text: Binding(
                        get: { appVM.connections.connectionFilter },
                        set: { appVM.connections.connectionFilter = $0 }
                    ))
                    .textFieldStyle(.plain)
                    .font(SbqlTheme.Typography.caption)
                    .foregroundStyle(SbqlTheme.Colors.textPrimary)

                    if !appVM.connections.connectionFilter.isEmpty {
                        Button {
                            appVM.connections.connectionFilter = ""
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
        let (label, color): (String, Color) = switch backend {
        case .postgres: ("PostgreSQL", Color(hex: 0x336791))
        case .mysql: ("MySQL", Color(hex: 0x00758F))
        case .sqlite: ("SQLite", Color(hex: 0x44A8D6))
        case .redis: ("Redis", Color(hex: 0xD82C20))
        case .dynamodb: ("DynamoDB", Color(hex: 0x4053D6))
        case .mongodb: ("MongoDB", Color(hex: 0x47A248))
        case .sqlserver: ("SQL Server", Color(hex: 0xCC2927))
        }
        return HStack(spacing: SbqlTheme.Spacing.xs) {
            Circle()
                .fill(color)
                .frame(width: 6, height: 6)
            Text("\(label) (\(count))")
                .font(.system(size: 10, weight: .semibold))
                .foregroundStyle(color)
            Spacer()
        }
        .padding(.horizontal, SbqlTheme.Spacing.lg)
        .padding(.top, SbqlTheme.Spacing.sm)
        .padding(.bottom, SbqlTheme.Spacing.xxs)
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
