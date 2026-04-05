import SwiftUI

/// Reusable empty state placeholder with icon, title, and optional subtitle/action.
struct EmptyStateView: View {
    let icon: String
    let title: String
    var subtitle: String? = nil
    var actionLabel: String? = nil
    var action: (() -> Void)? = nil

    var body: some View {
        VStack(spacing: SbqlTheme.Spacing.sm) {
            Image(systemName: icon)
                .font(.system(size: 28))
                .foregroundStyle(SbqlTheme.Colors.accent.opacity(0.4))
            Text(title)
                .font(SbqlTheme.Typography.body)
                .foregroundStyle(SbqlTheme.Colors.textSecondary)
            if let subtitle {
                Text(subtitle)
                    .font(SbqlTheme.Typography.caption)
                    .foregroundStyle(SbqlTheme.Colors.textTertiary)
            }
            if let actionLabel, let action {
                Button(actionLabel, action: action)
                    .font(SbqlTheme.Typography.captionBold)
                    .foregroundStyle(SbqlTheme.Colors.accent)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
