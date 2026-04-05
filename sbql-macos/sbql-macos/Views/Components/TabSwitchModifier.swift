import SwiftUI

/// ViewModifier that animates tab switches using direct state manipulation.
/// More reliable than `.id()` + `.transition()` which is inconsistent in many contexts.
struct TabSwitchModifier: ViewModifier {
    let tabId: String
    let direction: Edge
    let animation: TabAnimation

    @State private var offset: CGFloat = 0
    @State private var opacity: Double = 1
    @State private var scale: CGFloat = 1
    @State private var rotation: Double = 0
    @State private var isAnimating = false

    func body(content: Content) -> some View {
        content
            .offset(x: offset)
            .opacity(opacity)
            .scaleEffect(scale)
            .rotation3DEffect(.degrees(rotation), axis: (x: 0, y: 1, z: 0), perspective: 0.4)
            .onChange(of: tabId) { _, _ in
                guard animation != .none else { return }
                animateTransition()
            }
    }

    private func animateTransition() {
        guard !isAnimating else { return }
        isAnimating = true

        let slideDir: CGFloat = direction == .trailing ? -1 : 1

        switch animation {
        case .none:
            break

        case .fade:
            withAnimation(.easeIn(duration: 0.12)) {
                opacity = 0
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.12) {
                withAnimation(.easeOut(duration: 0.18)) {
                    opacity = 1
                }
                isAnimating = false
            }

        case .slide:
            withAnimation(.easeIn(duration: 0.12)) {
                offset = slideDir * 60
                opacity = 0
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.12) {
                offset = slideDir * -60
                withAnimation(.spring(duration: 0.25, bounce: 0.1)) {
                    offset = 0
                    opacity = 1
                }
                isAnimating = false
            }

        case .scaleBlur:
            withAnimation(.easeIn(duration: 0.12)) {
                scale = 0.95
                opacity = 0
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.12) {
                scale = 1.03
                withAnimation(.spring(duration: 0.25, bounce: 0.08)) {
                    scale = 1
                    opacity = 1
                }
                isAnimating = false
            }

        case .flip:
            withAnimation(.easeIn(duration: 0.15)) {
                rotation = slideDir * 90
                opacity = 0
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) {
                rotation = slideDir * -90
                withAnimation(.spring(duration: 0.3, bounce: 0.05)) {
                    rotation = 0
                    opacity = 1
                }
                isAnimating = false
            }

        case .dissolve:
            withAnimation(.easeIn(duration: 0.1)) {
                offset = slideDir * 10
                scale = 0.98
                opacity = 0
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                offset = slideDir * -10
                scale = 0.98
                withAnimation(.easeOut(duration: 0.2)) {
                    offset = 0
                    scale = 1
                    opacity = 1
                }
                isAnimating = false
            }
        }
    }
}
