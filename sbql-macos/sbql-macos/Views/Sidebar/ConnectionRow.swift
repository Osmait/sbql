import SwiftUI

struct ConnectionRow: View {
    let connection: Connection
    @Environment(AppViewModel.self) private var appVM

    private var isSelected: Bool {
        appVM.connections.selectedConnectionId == connection.id
    }

    var body: some View {
        HStack(spacing: SbqlTheme.Spacing.sm) {
            Circle()
                .fill(connection.isConnected ? SbqlTheme.Colors.success : SbqlTheme.Colors.danger.opacity(0.5))
                .frame(width: 6, height: 6)
                .scaleEffect(connection.isConnected ? 1.0 : 0.8)
                .animation(SbqlTheme.Animations.spring, value: connection.isConnected)

            VStack(alignment: .leading, spacing: 1) {
                Text(connection.name.isEmpty ? "Unnamed" : connection.name)
                    .font(SbqlTheme.Typography.body)
                    .foregroundStyle(SbqlTheme.Colors.textPrimary)
                    .lineLimit(1)

                Text(connection.displaySubtitle)
                    .font(SbqlTheme.Typography.caption)
                    .foregroundStyle(SbqlTheme.Colors.textTertiary)
                    .lineLimit(1)
            }

            Spacer()

            if connection.requiresBiometric {
                Image(systemName: "touchid")
                    .font(.system(size: 9))
                    .foregroundStyle(SbqlTheme.Colors.accent.opacity(0.5))
            }

            BackendBadgeView(backend: connection.backend)
        }
        .padding(.horizontal, SbqlTheme.Spacing.sm)
        .padding(.vertical, SbqlTheme.Spacing.xs)
        .background(
            isSelected
                ? SbqlTheme.Colors.selection
                : Color.clear
        )
        .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium))
        .animation(SbqlTheme.Animations.quick, value: isSelected)
        .animation(SbqlTheme.Animations.gentle, value: connection.isConnected)
        .hoverHighlight()
        .contentShape(Rectangle())
        .onTapGesture {
            appVM.connections.selectedConnectionId = connection.id
            Task {
                if connection.isConnected {
                    // Already connected — just select it
                } else {
                    // Disconnect current if any, then connect this one
                    if let current = appVM.connections.activeConnection,
                       current.id != connection.id {
                        await appVM.disconnect(id: current.id)
                    }
                    await appVM.connect(id: connection.id)
                }
            }
        }
        .contextMenu {
            if connection.isConnected {
                Button("Disconnect") {
                    Task { await appVM.disconnect(id: connection.id) }
                }
            } else {
                Button("Connect") {
                    Task { await appVM.connect(id: connection.id) }
                }
            }

            Divider()

            Button("Edit...") {
                appVM.connections.editingConnection = connection
                appVM.connections.isShowingConnectionForm = true
            }

            Button("Delete", role: .destructive) {
                Task {
                    do {
                        try await appVM.connections.deleteConnection(id: connection.id)
                    } catch {
                        appVM.showError(error)
                    }
                }
            }
        }
    }
}
