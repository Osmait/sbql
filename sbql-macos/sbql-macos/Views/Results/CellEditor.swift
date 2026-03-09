import SwiftUI

/// Popover cell editor for inline editing of a single cell value.
struct CellEditor: View {
    let column: String
    let currentValue: String
    let onSave: (String) -> Void

    @State private var editedValue: String
    @Environment(\.dismiss) private var dismiss

    init(column: String, currentValue: String, onSave: @escaping (String) -> Void) {
        self.column = column
        self.currentValue = currentValue
        self.onSave = onSave
        _editedValue = State(initialValue: currentValue)
    }

    var body: some View {
        VStack(spacing: SbqlTheme.Spacing.sm) {
            Text(column)
                .font(SbqlTheme.Typography.captionBold)
                .foregroundStyle(SbqlTheme.Colors.textSecondary)
                .frame(maxWidth: .infinity, alignment: .leading)

            TextEditor(text: $editedValue)
                .font(SbqlTheme.Typography.code)
                .frame(width: 280, height: 80)
                .scrollContentBackground(.hidden)
                .background(SbqlTheme.Colors.surfaceElevated)
                .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium))

            HStack {
                Button("Cancel") { dismiss() }
                    .buttonStyle(.plain)
                    .foregroundStyle(SbqlTheme.Colors.textSecondary)

                Spacer()

                Button("Save") {
                    onSave(editedValue)
                    dismiss()
                }
                .buttonStyle(.borderedProminent)
                .tint(SbqlTheme.Colors.accent)
                .disabled(editedValue == currentValue)
            }
        }
        .padding(SbqlTheme.Spacing.md)
        .frame(width: 320)
    }
}
