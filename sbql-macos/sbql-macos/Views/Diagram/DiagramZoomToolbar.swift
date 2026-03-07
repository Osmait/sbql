import SwiftUI

/// Floating pill toolbar with zoom controls (bottom-right corner).
struct DiagramZoomToolbar: View {
    let zoomPercent: Int
    var onZoomIn: () -> Void = {}
    var onZoomOut: () -> Void = {}
    var onResetZoom: () -> Void = {}
    var onFitToScreen: () -> Void = {}

    var body: some View {
        HStack(spacing: 0) {
            toolbarButton(icon: "minus", action: onZoomOut)

            Divider()
                .frame(height: 16)
                .background(SbqlTheme.Colors.border)

            Button(action: onResetZoom) {
                Text("\(zoomPercent)%")
                    .font(SbqlTheme.Typography.captionBold)
                    .foregroundStyle(SbqlTheme.Colors.textSecondary)
                    .frame(width: 44)
            }
            .buttonStyle(.plain)

            Divider()
                .frame(height: 16)
                .background(SbqlTheme.Colors.border)

            toolbarButton(icon: "plus", action: onZoomIn)

            Divider()
                .frame(height: 16)
                .background(SbqlTheme.Colors.border)

            toolbarButton(icon: "arrow.up.left.and.arrow.down.right", action: onFitToScreen)
        }
        .frame(height: 32)
        .background(SbqlTheme.Colors.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.large))
        .overlay(
            RoundedRectangle(cornerRadius: SbqlTheme.Radius.large)
                .stroke(SbqlTheme.Colors.border, lineWidth: 1)
        )
        .shadow(color: .black.opacity(0.3), radius: 8, y: 4)
    }

    private func toolbarButton(icon: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: icon)
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(SbqlTheme.Colors.textSecondary)
                .frame(width: 32, height: 32)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}
