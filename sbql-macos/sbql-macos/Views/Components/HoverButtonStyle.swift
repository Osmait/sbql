import SwiftUI

/// A plain button style that adds subtle hover feedback (background highlight + slight scale).
/// Use `.buttonStyle(.hover)` on any plain button to make it feel interactive.
struct HoverButtonStyle: ButtonStyle {
    @State private var isHovered = false

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .background(
                isHovered
                    ? SbqlTheme.Colors.surfaceHover.opacity(0.5)
                    : Color.clear
            )
            .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))
            .scaleEffect(configuration.isPressed ? 0.96 : 1.0)
            .opacity(configuration.isPressed ? 0.8 : 1.0)
            .animation(.easeOut(duration: 0.1), value: configuration.isPressed)
            .animation(.easeOut(duration: 0.15), value: isHovered)
            .onHover { isHovered = $0 }
    }
}

/// Icon-only button style with circular hover highlight.
struct HoverIconButtonStyle: ButtonStyle {
    @State private var isHovered = false

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .padding(4)
            .background(
                isHovered
                    ? SbqlTheme.Colors.surfaceHover.opacity(0.6)
                    : Color.clear
            )
            .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))
            .scaleEffect(configuration.isPressed ? 0.9 : isHovered ? 1.05 : 1.0)
            .animation(.easeOut(duration: 0.1), value: configuration.isPressed)
            .animation(.easeOut(duration: 0.15), value: isHovered)
            .onHover { isHovered = $0 }
    }
}

extension ButtonStyle where Self == HoverButtonStyle {
    static var hover: HoverButtonStyle { HoverButtonStyle() }
}

extension ButtonStyle where Self == HoverIconButtonStyle {
    static var hoverIcon: HoverIconButtonStyle { HoverIconButtonStyle() }
}
