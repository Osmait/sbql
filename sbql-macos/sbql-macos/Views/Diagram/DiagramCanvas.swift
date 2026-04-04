import SwiftUI

/// Integration layer: FK lines on a Canvas underneath positioned table nodes.
struct DiagramCanvas: View {
    @Environment(AppViewModel.self) private var appVM
    @State private var dragStart: [String: CGPoint] = [:]
    @State private var viewportSize: CGSize = .zero

    private var diagram: DiagramViewModel {
        appVM.diagram
    }

    var body: some View {
        GeometryReader { geo in
            let edges = DiagramEdgeGeometry.edges(
                for: diagram.diagramData.foreignKeys,
                tables: diagram.diagramData.tables,
                positions: diagram.tablePositions,
                hoveredConstraint: diagram.hoveredFkConstraint
            )

            let bounds = diagram.computeContentBounds()
            let canvasWidth = max(geo.size.width, bounds.maxX + 200)
            let canvasHeight = max(geo.size.height, bounds.maxY + 200)
            let scaledWidth = canvasWidth * diagram.scale
            let scaledHeight = canvasHeight * diagram.scale

            ScrollView([.horizontal, .vertical]) {
                ZStack(alignment: .topLeading) {
                    // FK lines (underneath)
                    Canvas { context, _ in
                        drawEdges(context: &context, edges: edges)
                    }

                    // Table nodes
                    ForEach(diagram.diagramData.tables) { table in
                        if let pos = diagram.tablePositions[table.id] {
                            DiagramTableNode(
                                table: table,
                                isSelected: diagram.selectedTableId == table.id,
                                isHovered: diagram.hoveredTableId == table.id,
                                hoveredFkConstraint: diagram.hoveredFkConstraint,
                                fksForTable: fksForTable(table.id),
                                onSelect: { selectTable(table.id) },
                                onHoverChange: { hovering in
                                    diagram.hoveredTableId = hovering ? table.id : nil
                                },
                                onDragChanged: { translation in
                                    handleDragChanged(tableId: table.id, translation: translation)
                                },
                                onDragEnded: {
                                    dragStart.removeValue(forKey: table.id)
                                }
                            )
                            .position(
                                x: pos.x + DiagramLayout.nodeWidth / 2,
                                y: pos.y + nodeHeight(for: table) / 2
                            )
                        }
                    }

                    // Invisible hover detection layer for FK lines
                    ForEach(edges) { edge in
                        bezierHitArea(for: edge)
                    }
                }
                .frame(width: canvasWidth, height: canvasHeight)
                .scaleEffect(diagram.scale, anchor: .topLeading)
                .frame(width: scaledWidth, height: scaledHeight, alignment: .topLeading)
            }
            .gesture(
                MagnifyGesture()
                    .onChanged { value in
                        diagram.scale = max(0.2, min(3.0, value.magnification))
                    }
            )
            .onAppear {
                viewportSize = geo.size
                diagram.computeInitialLayout()
                // Auto-fit to viewport after layout
                DispatchQueue.main.async {
                    diagram.fitToScreen(viewportSize: geo.size)
                }
            }
            .onChange(of: geo.size) { _, newSize in
                viewportSize = newSize
            }
        }
    }

    // MARK: - Edge Drawing

    private func drawEdges(context: inout GraphicsContext, edges: [DiagramEdge]) {
        let anyEdgeHovered = edges.contains(where: { $0.isHovered })
        let hoveredTable = diagram.hoveredTableId
        let anyHighlight = anyEdgeHovered || hoveredTable != nil
        let palette = SbqlTheme.Colors.fkLinePalette

        for edge in edges {
            let path = DiagramEdgeGeometry.bezierPath(for: edge)
            let baseColor = palette[edge.colorIndex]
            let color: Color
            let lineWidth: CGFloat

            // Edge is directly hovered
            let isEdgeActive = edge.isHovered
            // Edge connects to the hovered table
            let isTableActive = hoveredTable != nil &&
                (edge.fromTableId == hoveredTable || edge.toTableId == hoveredTable)

            if isEdgeActive {
                color = baseColor
                lineWidth = 2.5
            } else if isTableActive {
                color = baseColor.opacity(0.85)
                lineWidth = 2.0
            } else if anyHighlight {
                // Dim non-active lines when something is highlighted
                color = baseColor.opacity(0.10)
                lineWidth = 1.0
            } else {
                color = baseColor.opacity(0.55)
                lineWidth = 1.5
            }

            context.stroke(path, with: .color(color), lineWidth: lineWidth)

            // Source marker (one-tick)
            let sourcePath = DiagramEdgeGeometry.sourceMarkerPath(for: edge)
            context.stroke(sourcePath, with: .color(color), lineWidth: lineWidth)

            // Target marker (crow's foot)
            let crowsPath = DiagramEdgeGeometry.crowsFootPath(for: edge)
            context.stroke(crowsPath, with: .color(color), lineWidth: lineWidth)
        }
    }

    // MARK: - Hit Testing

    private func bezierHitArea(for edge: DiagramEdge) -> some View {
        let path = DiagramEdgeGeometry.bezierPath(for: edge)
        return path
            .stroke(Color.clear, lineWidth: 12) // fat invisible stroke for hit testing
            .contentShape(path.strokedPath(StrokeStyle(lineWidth: 12)))
            .onHover { hovering in
                diagram.hoveredFkConstraint = hovering ? edge.id : nil
            }
    }

    // MARK: - Helpers

    private func selectTable(_ id: String) {
        diagram.selectedTableId = diagram.selectedTableId == id ? nil : id
    }

    private func handleDragChanged(tableId: String, translation: CGSize) {
        if dragStart[tableId] == nil {
            dragStart[tableId] = diagram.tablePositions[tableId]
        }
        guard let start = dragStart[tableId] else { return }
        diagram.tablePositions[tableId] = CGPoint(
            x: start.x + translation.width,
            y: start.y + translation.height
        )
    }

    private func fksForTable(_ tableId: String) -> [DiagramForeignKey] {
        diagram.diagramData.foreignKeys.filter {
            tableId == "\($0.fromSchema).\($0.fromTable)" ||
                tableId == "\($0.toSchema).\($0.toTable)"
        }
    }

    private func nodeHeight(for table: DiagramTable) -> CGFloat {
        DiagramLayout.headerHeight + CGFloat(table.columns.count) * DiagramLayout.rowHeight
    }
}
