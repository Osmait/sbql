import SwiftUI

struct ToastNotification: View {
    let message: String
    var isError: Bool = false

    var body: some View {
        Text(message)
            .font(SbqlTheme.Typography.caption)
            .foregroundStyle(SbqlTheme.Colors.textPrimary)
            .padding(.horizontal, SbqlTheme.Spacing.lg)
            .padding(.vertical, SbqlTheme.Spacing.sm)
            .background(
                (isError ? SbqlTheme.Colors.danger : SbqlTheme.Colors.surfaceElevated)
                    .opacity(0.95)
            )
            .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.large))
            .shadow(color: .black.opacity(0.3), radius: 8, y: 4)
    }
}
