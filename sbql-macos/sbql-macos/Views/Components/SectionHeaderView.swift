import SwiftUI

/// Reusable section header with title, optional action button, and accent styling.
struct SectionHeaderView: View {
    let title: String
    var action: (() -> Void)? = nil
    var actionIcon: String = "plus"

    var body: some View {
        HStack {
            Text(title)
                .font(SbqlTheme.Typography.captionBold)
                .foregroundStyle(SbqlTheme.Colors.accent.opacity(0.7))
            Spacer()
            if let action {
                Button(action: action) {
                    Image(systemName: actionIcon)
                        .font(.system(size: 10, weight: .bold))
                        .foregroundStyle(SbqlTheme.Colors.accent)
                }
                .buttonStyle(.hoverIcon)
            }
        }
        .padding(.horizontal, SbqlTheme.Spacing.lg)
        .padding(.vertical, SbqlTheme.Spacing.xs)
    }
}
