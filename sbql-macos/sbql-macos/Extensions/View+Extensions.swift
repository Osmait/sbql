import SwiftUI
import AppKit

// MARK: - Window Accessor

/// Finds the hosting NSWindow and applies configuration.
struct WindowAccessor: NSViewRepresentable {
    let configure: (NSWindow) -> Void

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        DispatchQueue.main.async {
            if let window = view.window {
                self.configure(window)
            }
        }
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        if let window = nsView.window {
            configure(window)
        }
    }
}

extension View {
    /// Apply the sbql surface card style.
    func sbqlCard() -> some View {
        self
            .background(SbqlTheme.Colors.surface)
            .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.large))
            .overlay(
                RoundedRectangle(cornerRadius: SbqlTheme.Radius.large)
                    .stroke(SbqlTheme.Colors.border, lineWidth: 1)
            )
    }

    /// Conditional modifier.
    @ViewBuilder
    func `if`<Transform: View>(
        _ condition: Bool,
        transform: (Self) -> Transform
    ) -> some View {
        if condition {
            transform(self)
        } else {
            self
        }
    }
}
