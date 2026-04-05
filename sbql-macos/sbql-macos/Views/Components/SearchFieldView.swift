import SwiftUI

/// Reusable search field with magnifying glass icon, clear button, and themed styling.
struct SearchFieldView: View {
    @Binding var text: String
    let placeholder: String
    var font: Font = SbqlTheme.Typography.caption
    var iconSize: CGFloat = 10
    var icon: String = "magnifyingglass"
    var iconColor: Color = SbqlTheme.Colors.textTertiary

    var body: some View {
        HStack(spacing: SbqlTheme.Spacing.xs) {
            Image(systemName: icon)
                .font(.system(size: iconSize))
                .foregroundStyle(iconColor)
            TextField(placeholder, text: $text)
                .textFieldStyle(.plain)
                .font(font)
                .foregroundStyle(SbqlTheme.Colors.textPrimary)
            if !text.isEmpty {
                Button { text = "" } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 10))
                        .foregroundStyle(SbqlTheme.Colors.textTertiary)
                }
                .buttonStyle(.hoverIcon)
            }
        }
        .padding(.horizontal, SbqlTheme.Spacing.sm)
        .padding(.vertical, SbqlTheme.Spacing.xs)
        .background(SbqlTheme.Colors.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))
    }
}
