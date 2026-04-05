import SwiftUI

/// Labeled form field row with consistent styling.
struct FormFieldRow<Content: View>: View {
    let label: String
    var labelWidth: CGFloat = 80
    @ViewBuilder let content: () -> Content

    var body: some View {
        HStack {
            Text(label)
                .font(SbqlTheme.Typography.bodyMedium)
                .foregroundStyle(SbqlTheme.Colors.textSecondary)
                .frame(width: labelWidth, alignment: .leading)
            content()
        }
    }
}

/// Styled text input for form fields.
struct FormTextField: View {
    let placeholder: String
    @Binding var text: String
    var isSecure: Bool = false

    var body: some View {
        Group {
            if isSecure {
                SecureField(placeholder, text: $text)
            } else {
                TextField(placeholder, text: $text)
            }
        }
        .textFieldStyle(.plain)
        .font(SbqlTheme.Typography.body)
        .padding(SbqlTheme.Spacing.sm)
        .background(SbqlTheme.Colors.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium))
    }
}
