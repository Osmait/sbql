import SwiftUI

/// A view modifier that adds hover highlight to any view.
struct HoverHighlight: ViewModifier {
    @State private var isHovered = false

    func body(content: Content) -> some View {
        content
            .background(
                isHovered
                    ? SbqlTheme.Colors.surfaceHover.opacity(0.4)
                    : Color.clear
            )
            .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium))
            .animation(.easeOut(duration: 0.12), value: isHovered)
            .onHover { isHovered = $0 }
    }
}

extension View {
    func hoverHighlight() -> some View {
        modifier(HoverHighlight())
    }
}
