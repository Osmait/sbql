import SwiftUI

/// Reusable modal overlay that dims the background and dismisses on tap outside.
struct ModalPresenter<Content: View>: View {
    @Binding var isPresented: Bool
    @ViewBuilder let content: () -> Content

    var body: some View {
        if isPresented {
            ZStack {
                Color.black.opacity(0.4)
                    .ignoresSafeArea()
                    .onTapGesture {
                        withAnimation(SbqlTheme.Animations.quick) {
                            isPresented = false
                        }
                    }

                content()
                    .clipShape(RoundedRectangle(cornerRadius: 10))
                    .shadow(color: .black.opacity(0.4), radius: 20, y: 8)
            }
            .transition(.opacity)
            .animation(SbqlTheme.Animations.gentle, value: isPresented)
        }
    }
}
