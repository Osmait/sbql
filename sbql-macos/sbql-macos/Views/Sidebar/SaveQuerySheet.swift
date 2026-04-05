import SwiftUI

struct SaveQuerySheet: View {
    @Environment(AppViewModel.self) private var appVM
    @Environment(\.dismiss) private var dismiss

    @State private var name: String = ""

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Text("Save Query")
                    .font(SbqlTheme.Typography.title)
                    .foregroundStyle(SbqlTheme.Colors.textPrimary)
                Spacer()
                Button("Cancel") { dismiss() }
                    .buttonStyle(.hover)
                    .foregroundStyle(SbqlTheme.Colors.textSecondary)
            }
            .padding(SbqlTheme.Spacing.lg)

            Divider().background(SbqlTheme.Colors.border)

            VStack(spacing: SbqlTheme.Spacing.lg) {
                // Name field
                HStack {
                    Text("Name")
                        .font(SbqlTheme.Typography.bodyMedium)
                        .foregroundStyle(SbqlTheme.Colors.textSecondary)
                        .frame(width: 80, alignment: .leading)

                    TextField("My Query", text: $name)
                        .textFieldStyle(.plain)
                        .font(SbqlTheme.Typography.body)
                        .padding(SbqlTheme.Spacing.sm)
                        .background(SbqlTheme.Colors.surfaceElevated)
                        .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium))
                }

                // SQL preview (read-only)
                VStack(alignment: .leading, spacing: SbqlTheme.Spacing.xs) {
                    Text("SQL")
                        .font(SbqlTheme.Typography.bodyMedium)
                        .foregroundStyle(SbqlTheme.Colors.textSecondary)

                    Text(appVM.savedQueries.saveSheetSQL)
                        .font(SbqlTheme.Typography.codeSmall)
                        .foregroundStyle(SbqlTheme.Colors.textTertiary)
                        .lineLimit(5)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(SbqlTheme.Spacing.sm)
                        .background(SbqlTheme.Colors.surfaceElevated)
                        .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium))
                }
            }
            .padding(SbqlTheme.Spacing.lg)

            Spacer()

            Divider().background(SbqlTheme.Colors.border)

            // Actions
            HStack {
                Spacer()
                Button("Save") {
                    appVM.savedQueries.save(name: name, sql: appVM.savedQueries.saveSheetSQL)
                    dismiss()
                }
                .buttonStyle(.borderedProminent)
                .tint(SbqlTheme.Colors.accent)
                .disabled(name.isEmpty)
            }
            .padding(SbqlTheme.Spacing.lg)
        }
        .frame(width: 400, height: 320)
        .background(SbqlTheme.Colors.surface)
    }
}
