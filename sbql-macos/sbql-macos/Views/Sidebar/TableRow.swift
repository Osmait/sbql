import SwiftUI

struct TableRow: View {
    let table: TableEntryModel
    @Environment(AppViewModel.self) private var appVM

    private var isSelected: Bool {
        appVM.connections.selectedTable == table
    }

    var body: some View {
        HStack(spacing: SbqlTheme.Spacing.sm) {
            Image(systemName: "tablecells")
                .font(.system(size: 11))
                .foregroundStyle(SbqlTheme.Colors.textTertiary)

            Text(table.name)
                .font(SbqlTheme.Typography.body)
                .foregroundStyle(SbqlTheme.Colors.textPrimary)
                .lineLimit(1)

            Spacer()

            Text(table.schema)
                .font(SbqlTheme.Typography.caption)
                .foregroundStyle(SbqlTheme.Colors.textTertiary)
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
        .onTapGesture {
            appVM.connections.selectedTable = table
            Task { await appVM.selectTable(table) }
        }
    }
}
