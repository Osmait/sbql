import SwiftUI

struct LoadingOverlay: View {
    var message: String = "Loading..."

    var body: some View {
        ZStack {
            SbqlTheme.Colors.background.opacity(0.6)

            VStack(spacing: SbqlTheme.Spacing.md) {
                ProgressView()
                    .progressViewStyle(.circular)
                    .scaleEffect(0.8)
                    .tint(SbqlTheme.Colors.accent)

                Text(message)
                    .font(SbqlTheme.Typography.caption)
                    .foregroundStyle(SbqlTheme.Colors.textSecondary)
            }
            .padding(SbqlTheme.Spacing.xl)
            .background(SbqlTheme.Colors.surfaceElevated)
            .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.large))
        }
    }
}
