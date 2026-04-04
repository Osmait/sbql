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

            backendBadge
        }
        .padding(.horizontal, SbqlTheme.Spacing.sm)
        .padding(.vertical, SbqlTheme.Spacing.xs)
        .background(
            isSelected
                ? SbqlTheme.Colors.selection
                : Color.clear
        )
        .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium))
        .contentShape(Rectangle())
        .onTapGesture(count: 2) {
            Task {
                if connection.isConnected {
                    await appVM.disconnect(id: connection.id)
                } else {
                    await appVM.connect(id: connection.id)
                }
            }
        }
        .onTapGesture {
            appVM.connections.selectedConnectionId = connection.id
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
                    try? await appVM.connections.deleteConnection(id: connection.id)
                }
            }
        }
    }

    private var backendBadge: some View {
        let (label, color): (String, Color) = switch connection.backend {
        case .postgres: ("PG", Color(hex: 0x336791))
        case .mysql: ("MY", Color(hex: 0x00758F))
        case .sqlite: ("SQ", Color(hex: 0x44A8D6))
        case .redis: ("RD", Color(hex: 0xD82C20))
        case .dynamodb: ("DB", Color(hex: 0x4053D6))
        }
        return Text(label)
            .font(.system(size: 9, weight: .bold, design: .monospaced))
            .foregroundStyle(color)
            .padding(.horizontal, 4)
            .padding(.vertical, 2)
            .background(color.opacity(0.15))
            .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))
    }
}
