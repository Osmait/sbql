import SwiftUI

/// Thin shell: loading/empty/canvas states with zoom toolbar overlay.
struct DiagramView: View {
    @Environment(AppViewModel.self) private var appVM
    @State private var viewportSize: CGSize = .zero

    var body: some View {
        let diagram = appVM.diagram

        GeometryReader { geo in
            ZStack {
                SbqlTheme.Colors.background.ignoresSafeArea()

                if diagram.isLoading {
                    LoadingOverlay(message: "Loading diagram...")
                        .transition(.opacity)
                } else if diagram.diagramData.tables.isEmpty {
                    emptyState
                        .transition(.opacity)
                } else {
                    ZStack(alignment: .bottomTrailing) {
                        DiagramCanvas()

                        DiagramZoomToolbar(
                            zoomPercent: diagram.zoomPercent,
                            onZoomIn: { diagram.zoomIn() },
                            onZoomOut: { diagram.zoomOut() },
                            onResetZoom: { diagram.scale = 1.0 },
                            onFitToScreen: { diagram.fitToScreen(viewportSize: viewportSize) }
                        )
                        .padding(SbqlTheme.Spacing.lg)
                    }
                    .transition(.opacity)
                }
            }
            .animation(SbqlTheme.Animations.smooth, value: diagram.isLoading)
            .animation(SbqlTheme.Animations.smooth, value: diagram.diagramData.tables.count)
            .onAppear { viewportSize = geo.size }
            .onChange(of: geo.size) { _, newSize in viewportSize = newSize }
        }
        .onChange(of: diagram.diagramData.tables.count) {
            diagram.computeInitialLayout()
            DispatchQueue.main.async {
                diagram.fitToScreen(viewportSize: viewportSize)
            }
        }
    }

    private var emptyState: some View {
        VStack(spacing: SbqlTheme.Spacing.md) {
            Image(systemName: "rectangle.3.group")
                .font(.system(size: 32))
                .foregroundStyle(SbqlTheme.Colors.textTertiary)

            Text("Connect and switch to Diagram tab to see the ER diagram")
                .font(SbqlTheme.Typography.body)
                .foregroundStyle(SbqlTheme.Colors.textSecondary)

            Button("Load Diagram") {
                Task { await appVM.loadDiagram() }
            }
            .buttonStyle(.borderedProminent)
            .tint(SbqlTheme.Colors.accent)
        }
    }
}
