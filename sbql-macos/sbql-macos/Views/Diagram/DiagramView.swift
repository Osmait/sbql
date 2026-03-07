import SwiftUI

/// Thin shell: loading/empty/canvas states with zoom toolbar overlay.
struct DiagramView: View {
    @Environment(AppViewModel.self) private var appVM

    var body: some View {
        let diagram = appVM.diagram

        ZStack {
            SbqlTheme.Colors.background.ignoresSafeArea()

            if diagram.isLoading {
                LoadingOverlay(message: "Loading diagram...")
            } else if diagram.diagramData.tables.isEmpty {
                emptyState
            } else {
                ZStack(alignment: .bottomTrailing) {
                    DiagramCanvas()

                    DiagramZoomToolbar(
                        zoomPercent: diagram.zoomPercent,
                        onZoomIn: { diagram.zoomIn() },
                        onZoomOut: { diagram.zoomOut() },
                        onResetZoom: { diagram.scale = 1.0 },
                        onFitToScreen: {
                            // Use a reasonable default; fitToScreen is called from GeometryReader in canvas
                        }
                    )
                    .padding(SbqlTheme.Spacing.lg)
                }
            }
        }
        .onChange(of: diagram.diagramData.tables.count) {
            diagram.computeInitialLayout()
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
