import SwiftUI

/// State for the ER diagram view.
@Observable
final class DiagramViewModel {
    var diagramData: DiagramModel = .empty
    var isLoading: Bool = false
    var scale: CGFloat = 1.0
    var offset: CGSize = .zero

    // Per-table positions (enables drag)
    var tablePositions: [String: CGPoint] = [:]

    // Selection / hover state
    var selectedTableId: String?
    var hoveredTableId: String?
    var hoveredFkConstraint: String?

    // MARK: - Layout

    /// BFS-based relationship-aware placement.
    /// FK-connected tables are placed adjacent; unconnected tables go at the bottom.
    func computeInitialLayout() {
        let tables = diagramData.tables
        let fks = diagramData.foreignKeys
        guard !tables.isEmpty else { return }

        let tableById = Dictionary(uniqueKeysWithValues: tables.map { ($0.id, $0) })

        // Build adjacency list
        var adjacency: [String: Set<String>] = [:]
        for t in tables { adjacency[t.id] = [] }
        for fk in fks {
            let fromId = "\(fk.fromSchema).\(fk.fromTable)"
            let toId = "\(fk.toSchema).\(fk.toTable)"
            if tableById[fromId] != nil && tableById[toId] != nil {
                adjacency[fromId, default: []].insert(toId)
                adjacency[toId, default: []].insert(fromId)
            }
        }

        // Separate connected vs unconnected
        let connectedIds = Set(adjacency.filter { !$0.value.isEmpty }.keys)
        let unconnectedIds = tables.map(\.id).filter { !connectedIds.contains($0) }

        // BFS to determine placement order for connected tables
        var visited = Set<String>()
        var orderedConnected: [String] = []

        // Start BFS from the most-connected table
        let sortedByDegree = connectedIds.sorted {
            (adjacency[$0]?.count ?? 0) > (adjacency[$1]?.count ?? 0)
        }

        for startId in sortedByDegree {
            guard !visited.contains(startId) else { continue }
            var queue = [startId]
            visited.insert(startId)
            while !queue.isEmpty {
                let current = queue.removeFirst()
                orderedConnected.append(current)
                let neighbors = (adjacency[current] ?? []).sorted()
                for neighbor in neighbors {
                    if !visited.contains(neighbor) {
                        visited.insert(neighbor)
                        queue.append(neighbor)
                    }
                }
            }
        }

        let spacingX: CGFloat = DiagramLayout.nodeWidth + 60
        let spacingY: CGFloat = 220
        let startX: CGFloat = 60
        let startY: CGFloat = 60
        let cols = max(1, Int(sqrt(Double(orderedConnected.count)).rounded(.up)))

        var positions: [String: CGPoint] = [:]

        // Place connected tables
        for (idx, tableId) in orderedConnected.enumerated() {
            let col = idx % cols
            let row = idx / cols
            let nodeHeight = nodeHeight(for: tableId)
            _ = nodeHeight  // used for future variable-height spacing
            positions[tableId] = CGPoint(
                x: startX + CGFloat(col) * spacingX,
                y: startY + CGFloat(row) * spacingY
            )
        }

        // Place unconnected tables below
        let connectedMaxY = positions.values.map(\.y).max() ?? startY
        let unconnectedStartY = connectedMaxY + spacingY
        let unconnectedCols = max(1, Int(sqrt(Double(unconnectedIds.count)).rounded(.up)))

        for (idx, tableId) in unconnectedIds.enumerated() {
            let col = idx % unconnectedCols
            let row = idx / unconnectedCols
            positions[tableId] = CGPoint(
                x: startX + CGFloat(col) * spacingX,
                y: unconnectedStartY + CGFloat(row) * spacingY
            )
        }

        tablePositions = positions
    }

    private func nodeHeight(for tableId: String) -> CGFloat {
        guard let table = diagramData.tables.first(where: { $0.id == tableId }) else {
            return DiagramLayout.headerHeight
        }
        return DiagramLayout.headerHeight + CGFloat(table.columns.count) * DiagramLayout.rowHeight
    }

    // MARK: - Drag

    func moveTable(id: String, by delta: CGSize) {
        guard var pos = tablePositions[id] else { return }
        pos.x += delta.width
        pos.y += delta.height
        tablePositions[id] = pos
    }

    // MARK: - Zoom

    func zoomIn() {
        scale = min(3.0, scale + 0.15)
    }

    func zoomOut() {
        scale = max(0.2, scale - 0.15)
    }

    func fitToScreen(viewportSize: CGSize) {
        let bounds = computeContentBounds()
        guard bounds.width > 0, bounds.height > 0 else { return }

        let padded = CGSize(
            width: bounds.width + 120,
            height: bounds.height + 120
        )
        let scaleX = viewportSize.width / padded.width
        let scaleY = viewportSize.height / padded.height
        scale = max(0.2, min(1.5, min(scaleX, scaleY)))
        offset = CGSize(
            width: -bounds.minX * scale + (viewportSize.width - bounds.width * scale) / 2,
            height: -bounds.minY * scale + (viewportSize.height - bounds.height * scale) / 2
        )
    }

    var zoomPercent: Int {
        Int((scale * 100).rounded())
    }

    func computeContentBounds() -> CGRect {
        guard !tablePositions.isEmpty else { return .zero }
        var minX = CGFloat.infinity
        var minY = CGFloat.infinity
        var maxX = -CGFloat.infinity
        var maxY = -CGFloat.infinity

        for (tableId, pos) in tablePositions {
            let h = nodeHeight(for: tableId)
            minX = min(minX, pos.x)
            minY = min(minY, pos.y)
            maxX = max(maxX, pos.x + DiagramLayout.nodeWidth)
            maxY = max(maxY, pos.y + h)
        }
        return CGRect(x: minX, y: minY, width: maxX - minX, height: maxY - minY)
    }
}
