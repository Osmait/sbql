import SwiftUI

struct BackendBadgeView: View {
    let backend: Connection.Backend

    var body: some View {
        Text(backend.displayLabel)
            .font(SbqlTheme.Typography.captionBold)
            .foregroundStyle(backend.color)
            .padding(.horizontal, SbqlTheme.Spacing.sm)
            .padding(.vertical, 2)
            .background(backend.color.opacity(0.15))
            .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))
    }
}
